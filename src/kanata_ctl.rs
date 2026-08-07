//! Safe Kanata enable/disable with passthrough proof + rollback watchdog.
//!
//! Product path: `sunghyun kanata enable --safe` (called by install.sh).
//! Bare `darwin-rebuild` keeps `services.sunghyun.kanata.enable = false`
//! so a rebuild alone cannot brick the keyboard.
//!
//! Proofs are mechanical (no owner typing, no GUI keystroke hacks): kanata's
//! own log must show the device grab + processing loop, the
//! VirtualHID output device must appear in `hidutil list`, and the kanata pid
//! must stay stable. Any failure at any stage rolls back to disabled.

use crate::assets;
use crate::bootstrap::sudo_keepalive::{run_root, run_sudo_n, SudoKeepAlive};
use crate::error::{ActionError, ActionResult};
use crate::headless;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const LABEL: &str = "com.anaclumos.kanata";
const PLIST: &str = "/Library/LaunchDaemons/com.anaclumos.kanata.plist";
const PLIST_DISABLED: &str = "/Library/LaunchDaemons/com.anaclumos.kanata.plist.disabled";
const VHID_DAEMON_LABEL: &str = "org.pqrs.service.daemon.Karabiner-VirtualHIDDevice-Daemon";
const VHIDS_DAEMON: &str = "/Library/Application Support/org.pqrs/Karabiner-DriverKit-VirtualHIDDevice/Applications/Karabiner-VirtualHIDDevice-Daemon.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Daemon";
const VHIDS_MANAGER: &str = "/Applications/.Karabiner-VirtualHIDDevice-Manager.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Manager";

const TEMP_OUT: &str = "/tmp/sunghyun-kanata-temp.out";
const TEMP_ERR: &str = "/tmp/sunghyun-kanata-temp.err";

pub const PASSTHROUGH_KBD: &str = include_str!("../assets/kanata-passthrough.kbd");

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn write_executable_sh(path: &Path, body: &str) -> Result<(), String> {
    fs::write(path, body).map_err(|e| format!("write {}: {e}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .map_err(|e| format!("stat {}: {e}", path.display()))?
            .permissions();
        // 0755: root must be able to read via `/bin/sh` even when owned by the user.
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).map_err(|e| format!("chmod {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Run a multi-command privileged sequence as ONE sudo invocation so a single
/// cached ticket (or a single owner-typed password) covers it. Never osascript
/// admin, never per-command prompt loops.
fn run_root_script(name: &str, body: &str) -> Result<(), String> {
    let path = std::env::temp_dir().join(format!("sunghyun-{name}.sh"));
    write_executable_sh(&path, body)?;
    run_root(&["/bin/sh", &path.to_string_lossy()])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KanataState {
    RunningDaemon,
    RunningOrphan,
    Disabled,
    Absent,
}

pub fn status() -> ActionResult {
    let state = probe_state();
    let plist = Path::new(PLIST).is_file();
    let disabled = Path::new(PLIST_DISABLED).is_file();
    let pids = kanata_pids();
    println!("kanata_state={state:?}");
    println!("plist_active={plist}");
    println!("plist_disabled={disabled}");
    println!("pids={}", pids.join(","));
    println!("vhid_daemon_running={}", vhid_daemon_running());
    println!("vhid_dext_activated={}", vhid_dext_activated());
    println!("vhid_output_device_present={}", vhid_output_device_present());
    println!(
        "input_monitoring={}",
        if pids.is_empty() {
            "unknown (kanata not running; TCC not readable without FDA)"
        } else {
            "granted (kanata process is up and holding the grab)"
        }
    );
    Ok(())
}

pub fn disable() -> ActionResult {
    let _sudo = SudoKeepAlive::acquire();
    // One privileged script: bootout + persistent disable override + kill +
    // plist -> .disabled. The launchctl override (2026-08-08 hardening) means
    // a bare plist rename back can never re-arm the daemon on boot without
    // the safe-enable gate, which runs `launchctl enable` itself.
    let body = format!(
        "#!/bin/sh\n\
         launchctl bootout system/{LABEL} 2>/dev/null || true\n\
         launchctl disable system/{LABEL} 2>/dev/null || true\n\
         pkill -x kanata 2>/dev/null || true\n\
         if [ -f {PLIST} ]; then mv -f {PLIST} {PLIST_DISABLED}; fi\n\
         exit 0\n"
    );
    run_root_script("kanata-disable", &body).map_err(ActionError::failed)?;
    // Belt and braces for non-root strays.
    let _ = Command::new("pkill").args(["-x", "kanata"]).status();
    eprintln!(
        "kanata disabled (daemon bootout; launchctl disable override; plist -> .disabled if present)"
    );
    Ok(())
}

/// Enable only after:
/// 1) kanata >= 1.12.0 (grab-without-output recovery fix, jtroo/kanata#1950)
/// 2) VirtualHID dext activated + daemon running
/// 3) `kanata --check` on passthrough + full configs
/// 4) staged start (passthrough -> full) with mechanical health proof each stage
/// 5) LaunchDaemon install; final proof + ~10s watchdog or automatic rollback
pub fn enable_safe() -> ActionResult {
    if headless::is_headless() {
        return Err(ActionError::skipped(
            "kanata enable --safe skipped (headless; no keyboard session to prove against)",
        ));
    }
    let home = directories::UserDirs::new()
        .map(|u| u.home_dir().to_path_buf())
        .ok_or_else(|| ActionError::failed("no home directory"))?;

    // sudo -v once + keeper; run_root falls back to one interactive prompt when
    // the ticket is not visible (owner allows rare prompts; spam is a bug).
    let _sudo = SudoKeepAlive::acquire();

    eprintln!("kanata: baseline keyboard-stack proof…");
    prove_baseline().map_err(|e| {
        ActionError::failed(format!(
            "keyboard stack unhealthy before kanata enable ({e}); refusing to grab keyboard"
        ))
    })?;

    ensure_vhid_stack()?;
    assets::materialize_runtime_config(&home).map_err(ActionError::failed)?;
    let cfg_dir = assets::config_dir(&home);
    let passthrough = cfg_dir.join("kanata-passthrough.kbd");
    assets::write_file(&passthrough, PASSTHROUGH_KBD, false).map_err(ActionError::failed)?;
    let full = cfg_dir.join("kanata.kbd");

    let kanata_bin = resolve_kanata_bin()?;
    ensure_min_version(&kanata_bin)?;
    check_cfg(&kanata_bin, &passthrough)?;
    check_cfg(&kanata_bin, &full)?;

    // Stage 0: passthrough (identity mapping) only.
    eprintln!("kanata: starting passthrough stage with rollback watchdog…");
    run_stage(&kanata_bin, &passthrough, "passthrough")?;

    // Stage 1: full config.
    eprintln!("kanata: starting full-config stage with rollback watchdog…");
    run_stage(&kanata_bin, &full, "full")?;

    // Stage 2: LaunchDaemon (RunAtLoad + KeepAlive) with post-bootstrap watchdog.
    eprintln!("kanata: installing LaunchDaemon…");
    let log_marks = launchd_log_marks(&home);
    if let Err(e) = install_launch_daemon(&home, &kanata_bin, &full) {
        emergency_rollback();
        return Err(ActionError::failed(format!(
            "LaunchDaemon install failed; rolled back: {e}"
        )));
    }
    if let Err(e) = prove_kanata_stage(&log_marks, Duration::from_secs(20)) {
        emergency_rollback();
        return Err(ActionError::failed(format!(
            "LaunchDaemon proof failed; rolled back: {e}"
        )));
    }
    // Watchdog: KeepAlive restart-loop / late output-backend death detection.
    if let Err(e) = watchdog_recheck(&log_marks, Duration::from_secs(10)) {
        emergency_rollback();
        return Err(ActionError::failed(format!(
            "LaunchDaemon watchdog failed; rolled back: {e}"
        )));
    }

    eprintln!("kanata: enabled safely (passthrough + full + launchd proofs passed)");
    Ok(())
}

/// Start kanata in temp (non-launchd) mode on `cfg`, prove health, stop it.
fn run_stage(bin: &Path, cfg: &Path, label: &str) -> ActionResult {
    if let Err(first) = start_kanata_temp(bin, cfg, true) {
        // Most common first-run failure: Input Monitoring not yet granted for
        // this kanata binary path. Owner policy (2026-08-07): open the exact
        // Settings pane, let the owner flip the toggle, and poll by retrying
        // the start. No instruction text, no agents, no stdin prompts.
        let first = first.to_string();
        if !first.contains("Input Monitoring") && !first.contains("not permitted") {
            emergency_rollback();
            return Err(ActionError::failed(format!(
                "{label} start failed; rolled back: {first}"
            )));
        }
        if headless::is_headless() {
            emergency_rollback();
            return Err(ActionError::skipped(format!(
                "{label} needs the Input Monitoring grant for {} (headless; converge on next run)",
                bin.display()
            )));
        }
        eprintln!(
            "kanata: Input Monitoring grant missing for {}; opening System Settings and waiting for the toggle…",
            bin.display()
        );
        let _ = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        // Poll by retrying the start (TCC settle window): silent sudo -n
        // retries only (never re-prompt sudo in a loop).
        let deadline = Instant::now() + Duration::from_secs(120);
        let mut last = first;
        let mut ok = false;
        while Instant::now() < deadline {
            match start_kanata_temp(bin, cfg, false) {
                Ok(()) => {
                    ok = true;
                    break;
                }
                Err(e) => {
                    last = e.to_string();
                    if last.contains("sudo -n") {
                        break;
                    }
                }
            }
            thread::sleep(Duration::from_secs(4));
        }
        if !ok {
            emergency_rollback();
            return Err(ActionError::failed(format!(
                "{label} start failed (often Input Monitoring); rolled back: {last}"
            )));
        }
    }
    let marks = vec![
        (PathBuf::from(TEMP_OUT), 0u64),
        (PathBuf::from(TEMP_ERR), 0u64),
    ];
    if let Err(e) = prove_kanata_stage(&marks, Duration::from_secs(20)) {
        emergency_rollback();
        return Err(ActionError::failed(format!(
            "{label} proof failed; rolled back: {e}"
        )));
    }
    stop_temp_kanata();
    Ok(())
}

fn resolve_kanata_bin() -> Result<PathBuf, ActionError> {
    for c in [
        "/opt/homebrew/bin/kanata",
        "/usr/local/bin/kanata",
        "/run/current-system/sw/bin/kanata",
    ] {
        let p = PathBuf::from(c);
        if p.is_file() {
            return Ok(p);
        }
    }
    which("kanata").map(PathBuf::from).ok_or_else(|| {
        ActionError::failed("kanata binary not found (brew install kanata / flake homebrew)")
    })
}

/// kanata < 1.12.0 predates the grab-without-output recovery fix
/// (jtroo/kanata#1792 / #1950) — exactly today's brick class. Refuse it.
fn ensure_min_version(bin: &Path) -> ActionResult {
    let out = Command::new(bin)
        .arg("--version")
        .output()
        .map_err(|e| ActionError::failed(format!("spawn kanata --version: {e}")))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let ver = parse_semver(&text)
        .ok_or_else(|| ActionError::failed(format!("cannot parse kanata version from: {text}")))?;
    if ver < (1, 12, 0) {
        return Err(ActionError::failed(format!(
            "kanata {}.{}.{} < 1.12.0 lacks the grab-without-output recovery fix (brick risk); upgrade first",
            ver.0, ver.1, ver.2
        )));
    }
    Ok(())
}

fn parse_semver(text: &str) -> Option<(u32, u32, u32)> {
    let token = text.split_whitespace().find(|t| {
        let mut parts = t.trim_start_matches('v').splitn(3, '.');
        matches!(
            (parts.next(), parts.next(), parts.next()),
            (Some(a), Some(b), Some(_)) if a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit())
        )
    })?;
    let mut parts = token.trim_start_matches('v').splitn(3, '.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = parts
        .next()?
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()?;
    Some((major, minor, patch))
}

fn ensure_vhid_stack() -> ActionResult {
    if !vhid_dext_activated() {
        if !Path::new(VHIDS_MANAGER).is_file() {
            return Err(ActionError::failed(
                "Karabiner-DriverKit-VirtualHIDDevice missing; install the v6.2.0 pkg first",
            ));
        }
        eprintln!("kanata: activating Karabiner VirtualHID dext…");
        let _ = run_root(&[VHIDS_MANAGER, "forceActivate"]);
    }
    if !vhid_daemon_running() {
        eprintln!("kanata: starting Karabiner-VirtualHIDDevice-Daemon…");
        // The pkg registers the daemon with ServiceManagement; kickstart it.
        let _ = run_root(&["launchctl", "kickstart", &format!("system/{VHID_DAEMON_LABEL}")]);
        for _ in 0..20 {
            if vhid_daemon_running() {
                break;
            }
            thread::sleep(Duration::from_millis(250));
        }
    }
    if !vhid_daemon_running() {
        // Last resort: start the daemon binary detached (never block on it).
        if Path::new(VHIDS_DAEMON).is_file() {
            let body = format!(
                "#!/bin/sh\nnohup {} >/dev/null 2>&1 &\nexit 0\n",
                shell_single_quote(VHIDS_DAEMON)
            );
            let _ = run_root_script("vhid-daemon-start", &body);
            for _ in 0..20 {
                if vhid_daemon_running() {
                    break;
                }
                thread::sleep(Duration::from_millis(250));
            }
        }
    }
    if !vhid_daemon_running() {
        return Err(ActionError::failed(
            "VirtualHID daemon not running; kanata would brick the keyboard (refusing enable)",
        ));
    }
    Ok(())
}

fn vhid_daemon_running() -> bool {
    Command::new("pgrep")
        .args(["-f", "Karabiner-VirtualHIDDevice-Daemon"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn vhid_dext_activated() -> bool {
    let out = Command::new("systemextensionsctl").arg("list").output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .any(|l| l.contains("org.pqrs.Karabiner-DriverKit-VirtualHIDDevice") && l.contains("activated")),
        Err(_) => false,
    }
}

/// The VirtualHID keyboard appears in the HID device tree only while a client
/// (kanata) is connected — presence proves the output path exists.
fn vhid_output_device_present() -> bool {
    let out = Command::new("hidutil").arg("list").output();
    match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            s.contains("VirtualHIDKeyboard") || s.contains("Karabiner DriverKit")
        }
        Err(_) => false,
    }
}

fn physical_keyboard_present() -> bool {
    let out = Command::new("hidutil").arg("list").output();
    match out {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .any(|l| l.contains("Keyboard") && !l.contains("VirtualHID")),
        Err(_) => false,
    }
}

fn check_cfg(bin: &Path, cfg: &Path) -> ActionResult {
    let out = Command::new(bin)
        .args(["--cfg", &cfg.to_string_lossy(), "--check"])
        .output()
        .map_err(|e| ActionError::failed(format!("spawn kanata --check: {e}")))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(ActionError::failed(format!(
            "kanata --check failed for {}: {}",
            cfg.display(),
            String::from_utf8_lossy(&out.stderr)
        )))
    }
}

fn start_kanata_temp(bin: &Path, cfg: &Path, allow_prompt: bool) -> Result<(), ActionError> {
    stop_temp_kanata();
    let _ = run_sudo_n(&["launchctl", "bootout", &format!("system/{LABEL}")]);
    // Root required for VirtualHID IPC under tmp/rootonly. Dedicated start
    // script (quoted paths), detached via nohup so sudo never blocks on it.
    let start_sh = std::env::temp_dir().join("sunghyun-kanata-temp-start.sh");
    let body = format!(
        "#!/bin/sh\nnohup {} --cfg {} --no-wait >{TEMP_OUT} 2>{TEMP_ERR} &\nexit 0\n",
        shell_single_quote(&bin.to_string_lossy()),
        shell_single_quote(&cfg.to_string_lossy()),
    );
    write_executable_sh(&start_sh, &body).map_err(ActionError::failed)?;
    let _ = fs::remove_file(TEMP_ERR);
    let _ = fs::remove_file(TEMP_OUT);
    let argv = ["/bin/sh", &start_sh.to_string_lossy() as &str];
    let started = if allow_prompt {
        run_root(&argv)
    } else {
        run_sudo_n(&argv)
    };
    started.map_err(ActionError::failed)?;
    thread::sleep(Duration::from_secs(2));
    if kanata_pids().is_empty() {
        let err = fs::read_to_string(TEMP_ERR).unwrap_or_default();
        return Err(ActionError::failed(format!(
            "kanata failed to stay up after start: {err}"
        )));
    }
    Ok(())
}

fn stop_temp_kanata() {
    let _ = run_sudo_n(&["pkill", "-x", "kanata"]);
    let _ = Command::new("pkill").args(["-x", "kanata"]).status();
    // Give the grab a moment to release before the next stage.
    for _ in 0..10 {
        if kanata_pids().is_empty() {
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn emergency_rollback() {
    eprintln!("kanata: ROLLBACK — disabling after failed proof");
    let _ = disable();
}

fn launch_daemon_plist_body(home: &Path, bin: &Path, cfg: &Path) -> String {
    let log_dir = home.join("Library/Logs/sunghyun");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{LABEL}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{}</string>
		<string>--cfg</string>
		<string>{}</string>
		<string>--no-wait</string>
	</array>
	<key>UserName</key>
	<string>root</string>
	<key>EnvironmentVariables</key>
	<dict>
		<key>PATH</key>
		<string>/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin</string>
	</dict>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<dict>
		<key>SuccessfulExit</key>
		<false/>
	</dict>
	<key>StandardOutPath</key>
	<string>{}/kanata.out.log</string>
	<key>StandardErrorPath</key>
	<string>{}/kanata.err.log</string>
</dict>
</plist>
"#,
        bin.display(),
        cfg.display(),
        log_dir.display(),
        log_dir.display()
    )
}

fn install_launch_daemon(home: &Path, bin: &Path, cfg: &Path) -> ActionResult {
    let log_dir = home.join("Library/Logs/sunghyun");
    let _ = fs::create_dir_all(&log_dir);
    let staged = home.join(".config/sunghyun/com.anaclumos.kanata.plist");
    assets::write_file(&staged, &launch_daemon_plist_body(home, bin, cfg), false)
        .map_err(ActionError::failed)?;
    stop_temp_kanata();
    // One privileged script = one sudo ticket use for the whole install.
    let body = format!(
        "#!/bin/sh\nset -e\n\
         launchctl bootout system/{LABEL} 2>/dev/null || true\n\
         launchctl enable system/{LABEL}\n\
         rm -f {PLIST_DISABLED}\n\
         cp -f {} {PLIST}\n\
         chown root:wheel {PLIST}\n\
         chmod 644 {PLIST}\n\
         launchctl bootstrap system {PLIST}\n",
        shell_single_quote(&staged.to_string_lossy()),
    );
    run_root_script("kanata-launchd-install", &body).map_err(ActionError::failed)
}

fn kanata_pids() -> Vec<String> {
    let out = Command::new("pgrep").args(["-x", "kanata"]).output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn probe_state() -> KanataState {
    let pids = kanata_pids();
    if pids.is_empty() {
        if Path::new(PLIST).is_file() || Path::new(PLIST_DISABLED).is_file() {
            return KanataState::Disabled;
        }
        return KanataState::Absent;
    }
    let loaded = Command::new("launchctl")
        .args(["print", &format!("system/{LABEL}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if loaded {
        KanataState::RunningDaemon
    } else {
        KanataState::RunningOrphan
    }
}

// --- mechanical proofs -------------------------------------------------------

/// kanata log lines that prove the grab + output loop are up.
const SUCCESS_MARKERS: &[&str] = &[
    "entering the processing loop",
    "keyboard grabbed, entering event processing loop",
];

/// Fatal immediately: permission / driver states that never self-heal within a
/// stage and are exactly the brick class (grab without healthy output).
const FATAL_MARKERS: &[&str] = &[
    "Input Monitoring permission is denied",
    "Input Monitoring permission not yet decided",
    "Accessibility permission",
    "IOHIDDeviceOpen error",
    "not permitted",
    "grab failed",
    "driver is not activated",
    "Couldn't register any device",
];

/// Fatal after startup (post-success watchdog): output backend loss.
const DEGRADED_MARKERS: &[&str] = &[
    "connect_failed",
    "output backend not ready",
    "output backend unavailable",
    "DriverKit virtual keyboard not ready",
];

fn read_new(path: &Path, offset: u64) -> String {
    let Ok(mut f) = fs::File::open(path) else {
        return String::new();
    };
    if f.seek(SeekFrom::Start(offset)).is_err() {
        return String::new();
    }
    let mut s = String::new();
    let _ = f.read_to_string(&mut s);
    s
}

fn find_marker(haystack: &str, markers: &[&str]) -> Option<String> {
    for m in markers {
        if haystack.contains(m) {
            return Some((*m).to_string());
        }
    }
    None
}

fn launchd_log_marks(home: &Path) -> Vec<(PathBuf, u64)> {
    let dir = home.join("Library/Logs/sunghyun");
    ["kanata.out.log", "kanata.err.log"]
        .iter()
        .map(|n| {
            let p = dir.join(n);
            let len = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            (p, len)
        })
        .collect()
}

/// Baseline (before any grab): a physical keyboard exists and nothing is
/// already seizing it. Requires no sudo and no owner typing.
fn prove_baseline() -> Result<(), String> {
    if !physical_keyboard_present() {
        return Err("no physical keyboard visible in hidutil list".into());
    }
    if !kanata_pids().is_empty() {
        // enable_safe restarts stages itself; a stray grab at baseline means
        // an unknown owner is holding the keyboard.
        return Err("kanata already running before enable (disable first)".into());
    }
    Ok(())
}

/// Prove a started kanata is healthy: success marker in its fresh log within
/// `budget`, no fatal marker, stable pid, VirtualHID output device present.
fn prove_kanata_stage(log_marks: &[(PathBuf, u64)], budget: Duration) -> Result<(), String> {
    let deadline = Instant::now() + budget;
    let combined = loop {
        let combined: String = log_marks
            .iter()
            .map(|(p, off)| read_new(p, *off))
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(m) = find_marker(&combined, FATAL_MARKERS) {
            return Err(format!("kanata log shows fatal condition: {m}"));
        }
        if find_marker(&combined, SUCCESS_MARKERS).is_some() {
            break combined;
        }
        if kanata_pids().is_empty() {
            return Err(format!(
                "kanata exited before reaching the processing loop; log tail: {}",
                tail(&combined, 400)
            ));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "no grab/processing-loop marker within {}s; log tail: {}",
                budget.as_secs(),
                tail(&combined, 400)
            ));
        }
        thread::sleep(Duration::from_millis(500));
    };

    // Output path: VirtualHID keyboard must be instantiated (client connected).
    let dev_deadline = Instant::now() + Duration::from_secs(8);
    while !vhid_output_device_present() {
        if Instant::now() >= dev_deadline {
            return Err("VirtualHID output keyboard did not appear in hidutil list".into());
        }
        thread::sleep(Duration::from_millis(500));
    }

    // Pid stability: same pid alive across a 2s window (no crash/restart loop).
    let pids_a = kanata_pids();
    if pids_a.is_empty() {
        return Err("kanata exited right after reporting the processing loop".into());
    }
    thread::sleep(Duration::from_secs(2));
    let pids_b = kanata_pids();
    if pids_a != pids_b {
        return Err(format!(
            "kanata pid churn (restart loop?): {pids_a:?} -> {pids_b:?}"
        ));
    }

    // No degraded-output markers after startup settled.
    let fresh: String = log_marks
        .iter()
        .map(|(p, off)| read_new(p, *off))
        .collect::<Vec<_>>()
        .join("\n");
    // connect_failed lines strictly before the success marker are startup
    // retries; anything after the marker means the output backend degraded.
    let after_success = SUCCESS_MARKERS
        .iter()
        .filter_map(|m| fresh.find(m).map(|i| i + m.len()))
        .max()
        .map(|i| &fresh[i..])
        .unwrap_or(&fresh);
    if let Some(m) = find_marker(after_success, DEGRADED_MARKERS) {
        return Err(format!("kanata output backend degraded after start: {m}"));
    }
    let _ = combined;
    Ok(())
}

/// Post-enable watchdog: after `settle`, the same pid must still be alive and
/// no fatal/degraded marker may have appeared.
fn watchdog_recheck(log_marks: &[(PathBuf, u64)], settle: Duration) -> Result<(), String> {
    let pids_before = kanata_pids();
    thread::sleep(settle);
    let pids_after = kanata_pids();
    if pids_after.is_empty() {
        return Err("kanata died within the watchdog window".into());
    }
    if pids_before != pids_after {
        return Err(format!(
            "kanata restarted within the watchdog window: {pids_before:?} -> {pids_after:?}"
        ));
    }
    let fresh: String = log_marks
        .iter()
        .map(|(p, off)| read_new(p, *off))
        .collect::<Vec<_>>()
        .join("\n");
    let after_success = SUCCESS_MARKERS
        .iter()
        .filter_map(|m| fresh.find(m).map(|i| i + m.len()))
        .max()
        .map(|i| &fresh[i..])
        .unwrap_or(&fresh);
    if let Some(m) = find_marker(after_success, FATAL_MARKERS) {
        return Err(format!("fatal condition during watchdog window: {m}"));
    }
    if let Some(m) = find_marker(after_success, DEGRADED_MARKERS) {
        return Err(format!("output backend degraded during watchdog window: {m}"));
    }
    Ok(())
}

fn tail(s: &str, n: usize) -> String {
    let start = s.len().saturating_sub(n);
    s[start..].to_string()
}

fn which(name: &str) -> Option<String> {
    let out = Command::new("sh")
        .args(["-lc", &format!("command -v {name}")])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_asset_is_identity_layer() {
        assert!(PASSTHROUGH_KBD.contains("process-unmapped-keys yes"));
        assert!(PASSTHROUGH_KBD.contains("lmet rmet"));
        assert!(!PASSTHROUGH_KBD.contains("tap-hold-press"));
        assert!(!PASSTHROUGH_KBD.contains("danger-enable-cmd yes"));
    }

    #[test]
    fn semver_parse_and_floor() {
        assert_eq!(parse_semver("kanata 1.12.0"), Some((1, 12, 0)));
        assert_eq!(parse_semver("kanata v1.11.0\n"), Some((1, 11, 0)));
        assert!(parse_semver("kanata 1.11.0").unwrap() < (1, 12, 0));
        assert!(parse_semver("kanata 1.12.0").unwrap() >= (1, 12, 0));
        assert_eq!(parse_semver("no version here"), None);
    }

    #[test]
    fn probe_handles_absent() {
        // Smoke: function returns without panic.
        let _ = probe_state();
    }

    #[test]
    fn shell_single_quote_escapes_apostrophe() {
        assert_eq!(shell_single_quote("a'b"), "'a'\\''b'");
        assert_eq!(shell_single_quote("/tmp/x"), "'/tmp/x'");
    }

    #[test]
    fn temp_start_script_quotes_paths() {
        let body = format!(
            "#!/bin/sh\nnohup {} --cfg {} --no-wait >{TEMP_OUT} 2>{TEMP_ERR} &\nexit 0\n",
            shell_single_quote("/opt/homebrew/bin/kanata"),
            shell_single_quote("/Users/sc/.config/sunghyun/kanata-passthrough.kbd"),
        );
        assert!(body.contains("'/opt/homebrew/bin/kanata'"));
        assert!(body.contains("'/Users/sc/.config/sunghyun/kanata-passthrough.kbd'"));
        assert!(!body.contains("sh -c nohup"));
    }

    #[test]
    fn no_textedit_or_admin_osascript_in_this_module() {
        let src = include_str!("kanata_ctl.rs");
        assert!(
            !src.contains(concat!("Text", "Edit")),
            "editor keystroke hack must stay dead"
        );
        assert!(
            !src.contains(concat!("administrator", " privileges")),
            "osascript admin prompts must stay dead"
        );
    }

    #[test]
    fn marker_scanning_finds_success_and_fatal() {
        let log = "07:00:00 [INFO] entering the processing loop\n";
        assert!(find_marker(log, SUCCESS_MARKERS).is_some());
        assert!(find_marker(log, FATAL_MARKERS).is_none());
        let bad = "IOHIDDeviceOpen error: (iokit/common) not permitted\n";
        assert!(find_marker(bad, FATAL_MARKERS).is_some());
    }

    #[test]
    fn degraded_after_success_detected() {
        let log = "connect_failed retry\nentering the processing loop\nconnect_failed again\n";
        let idx = log.find("entering the processing loop").unwrap()
            + "entering the processing loop".len();
        assert!(find_marker(&log[idx..], DEGRADED_MARKERS).is_some());
        let good = "connect_failed retry\nentering the processing loop\nall well\n";
        let idx2 = good.find("entering the processing loop").unwrap()
            + "entering the processing loop".len();
        assert!(find_marker(&good[idx2..], DEGRADED_MARKERS).is_none());
    }

    #[test]
    fn plist_body_points_at_full_config() {
        let body = launch_daemon_plist_body(
            Path::new("/Users/sc"),
            Path::new("/opt/homebrew/bin/kanata"),
            Path::new("/Users/sc/.config/sunghyun/kanata.kbd"),
        );
        assert!(body.contains("<string>/opt/homebrew/bin/kanata</string>"));
        assert!(body.contains("kanata.kbd"));
        assert!(body.contains("<key>KeepAlive</key>"));
        assert!(body.contains("<string>root</string>"));
    }
}
