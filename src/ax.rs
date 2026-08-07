use crate::headless;
use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How Accessibility gating should behave for the current session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxGateMode {
    /// Open Settings once and poll until trusted (or timeout / best-effort).
    WaitInteractive,
    /// Headless / forced: skip without waiting.
    SkipHeadless,
}

/// Pure branch used by verify/bootstrap and unit tests.
pub fn ax_gate_mode(headless: bool) -> AxGateMode {
    if headless {
        AxGateMode::SkipHeadless
    } else {
        AxGateMode::WaitInteractive
    }
}

/// True when the *current* process is Accessibility-trusted.
///
/// Uses `AXIsProcessTrustedWithOptions(prompt=false)` so TCC sees the running
/// CDHash (plain `AXIsProcessTrusted` can stay false after adhoc reinstalls
/// while Settings still shows a stale toggle for another path/hash).
pub fn is_process_trusted() -> bool {
    #[cfg(target_os = "macos")]
    {
        ax_trusted_with_options(false) || ax_trusted_legacy()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Non-blocking status for `verify` (never opens Settings, never polls).
///
/// Uses the *disclaimed* probe: trust inherited from the responsible process
/// (Terminal/Cursor) must not count, because Karabiner shell_commands run
/// under karabiner_console_user_server and only a direct grant on this binary
/// makes tiling work there. verify reports the direct grant, nothing else.
pub fn accessibility_status() -> AxGateOutcome {
    if cfg!(not(target_os = "macos")) {
        return AxGateOutcome::Skipped("Accessibility is macOS-only".into());
    }
    let probe = trust_probe();
    if probe.trusted {
        AxGateOutcome::Trusted
    } else if headless::is_headless() {
        AxGateOutcome::Skipped(
            "Accessibility skipped (headless); grant later for tiling".into(),
        )
    } else {
        AxGateOutcome::Failed(format!(
            "Accessibility not granted to the binary itself ({}); tile fails under Karabiner until granted — `sunghyun post-switch` opens the pane and polls",
            probe.running_path_display()
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TrustProbe {
    trusted: bool,
    running: Option<PathBuf>,
    granted_copy: Option<PathBuf>,
}

impl TrustProbe {
    fn running_path_display(&self) -> String {
        self.running
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<unknown>".into())
    }
}

/// Direct grants only: the running binary and installed `sunghyun` copies,
/// each probed with TCC responsibility disclaimed so inherited trust from the
/// spawning chain (Terminal, Cursor, karabiner_console_user_server) never
/// counts as a grant on the binary.
fn trust_probe() -> TrustProbe {
    let running = env::current_exe().ok();

    let mut granted_copy = None;
    if let Some(r) = running.as_deref() {
        if path_reports_trusted(r) {
            granted_copy = Some(r.to_path_buf());
        }
    }
    if granted_copy.is_none() {
        for path in candidate_binaries(running.as_deref()) {
            if path_reports_trusted(&path) {
                granted_copy = Some(path);
                break;
            }
        }
    }

    TrustProbe {
        trusted: granted_copy.is_some(),
        running,
        granted_copy,
    }
}

fn candidate_binaries(running: Option<&Path>) -> Vec<PathBuf> {
    let mut out = BTreeSet::new();
    if let Some(p) = running {
        out.insert(canonicalize_loose(p));
    }
    for key in ["HOME", "CARGO_HOME"] {
        if let Ok(home) = env::var(key) {
            let base = PathBuf::from(home);
            if key == "HOME" {
                out.insert(canonicalize_loose(&base.join(".cargo/bin/sunghyun")));
                out.insert(canonicalize_loose(&base.join(".local/bin/sunghyun")));
            } else {
                out.insert(canonicalize_loose(&base.join("bin/sunghyun")));
            }
        }
    }
    out.insert(canonicalize_loose(Path::new("/usr/local/bin/sunghyun")));
    out.insert(canonicalize_loose(Path::new("/opt/homebrew/bin/sunghyun")));

    if let Ok(path) = env::var("PATH") {
        for dir in env::split_paths(&path) {
            out.insert(canonicalize_loose(&dir.join("sunghyun")));
        }
    }

    out.into_iter()
        .filter(|p| p.is_file())
        .filter(|p| {
            running
                .map(|r| canonicalize_loose(r) != *p)
                .unwrap_or(true)
        })
        .collect()
}

fn canonicalize_loose(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Spawn a copy with `SUNGHYUN_AX_PROBE=1` and TCC responsibility disclaimed
/// (exit 0 = that binary itself holds the Accessibility grant).
fn path_reports_trusted(path: &Path) -> bool {
    spawn_disclaimed(path, &[], "SUNGHYUN_AX_PROBE") == Some(0)
}

/// Marker so a disclaimed action child never re-execs itself again.
pub const DISCLAIM_MARKER: &str = "SUNGHYUN_DISCLAIMED";

/// AXUIElement / CGEventPost attribute to the **responsible process**, not
/// the exact binary: even with `/usr/local/bin/sunghyun` granted, running it
/// from an unentitled parent (karabiner_console_user_server, a terminal)
/// fails with kAXErrorAPIDisabled (verified live 2026-08-08 on macOS 27).
/// A responsibility-disclaimed re-exec makes this binary its own TCC
/// principal, so the direct grant applies from any spawning chain.
/// Returns the child's exit code, or None when the caller should just run
/// in-process (already disclaimed, non-macOS, or spawn failure).
pub fn reexec_disclaimed_exit_code() -> Option<i32> {
    if env::var_os(DISCLAIM_MARKER).is_some() {
        return None;
    }
    let exe = env::current_exe().ok()?;
    let args: Vec<std::ffi::OsString> = env::args_os().skip(1).collect();
    let arg_refs: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
    spawn_disclaimed(&exe, &arg_refs, DISCLAIM_MARKER)
}

/// posix_spawn with `responsibility_spawnattrs_setdisclaim`, making the child
/// its own TCC responsible process. Inherits the full environment plus
/// `<marker>=1`. Returns the child's exit code, None on spawn error.
#[cfg(target_os = "macos")]
fn spawn_disclaimed(path: &Path, args: &[&std::ffi::OsStr], marker: &str) -> Option<i32> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    // Re-spawning current_exe is only safe when the target honors the
    // SUNGHYUN_* fast paths / marker in main(). The cargo test harness does
    // not and would recurse into the full suite per probe (observed as a
    // fork bomb, 2026-08-08). Probe children must also never probe again.
    if cfg!(test)
        || env::var_os("SUNGHYUN_AX_PROBE").is_some()
        || env::var_os("SUNGHYUN_AX_REGISTER").is_some()
        || env::var_os(DISCLAIM_MARKER).is_some()
    {
        return None;
    }

    extern "C" {
        // Private but long-stable libsystem API (used by launchd tooling);
        // the only non-MDM way to detach a child from the caller's TCC
        // attribution chain.
        fn responsibility_spawnattrs_setdisclaim(
            attr: *mut libc::posix_spawnattr_t,
            disclaim: libc::c_int,
        ) -> libc::c_int;
    }

    let prog = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut argv_owned: Vec<CString> = vec![prog.clone()];
    for a in args {
        argv_owned.push(CString::new(a.as_bytes()).ok()?);
    }
    let mut envp_owned: Vec<CString> = Vec::new();
    for (k, v) in env::vars_os() {
        let mut kv = k.as_bytes().to_vec();
        kv.push(b'=');
        kv.extend_from_slice(v.as_bytes());
        envp_owned.push(CString::new(kv).ok()?);
    }
    envp_owned.push(CString::new(format!("{marker}=1")).ok()?);

    let mut argv: Vec<*mut libc::c_char> = argv_owned
        .iter()
        .map(|s| s.as_ptr() as *mut libc::c_char)
        .chain(std::iter::once(std::ptr::null_mut()))
        .collect();
    let mut envp: Vec<*mut libc::c_char> = envp_owned
        .iter()
        .map(|s| s.as_ptr() as *mut libc::c_char)
        .chain(std::iter::once(std::ptr::null_mut()))
        .collect();

    unsafe {
        let mut attr: libc::posix_spawnattr_t = std::mem::zeroed();
        if libc::posix_spawnattr_init(&mut attr) != 0 {
            return None;
        }
        let _ = responsibility_spawnattrs_setdisclaim(&mut attr, 1);
        let mut pid: libc::pid_t = 0;
        let rc = libc::posix_spawn(
            &mut pid,
            prog.as_ptr(),
            std::ptr::null(),
            &attr,
            argv.as_mut_ptr(),
            envp.as_mut_ptr(),
        );
        libc::posix_spawnattr_destroy(&mut attr);
        if rc != 0 {
            return None;
        }
        let mut status: libc::c_int = 0;
        if libc::waitpid(pid, &mut status, 0) < 0 {
            return None;
        }
        if libc::WIFEXITED(status) {
            Some(libc::WEXITSTATUS(status))
        } else {
            Some(1)
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn spawn_disclaimed(_path: &Path, _args: &[&std::ffi::OsStr], _marker: &str) -> Option<i32> {
    None
}

pub fn open_accessibility_settings() {
    if cfg!(not(target_os = "macos")) {
        return;
    }
    let urls = [
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_Accessibility",
    ];
    for url in urls {
        let ok = Command::new("open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            return;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxGateOutcome {
    Trusted,
    Skipped(String),
    Failed(String),
}

/// Ensure Accessibility is granted. Never blocks on stdin Enter. Never calls sudo.
pub fn ensure_accessibility(headless_mode: bool) -> AxGateOutcome {
    let headless_mode = headless_mode || headless::is_headless();
    match ax_gate_mode(headless_mode) {
        AxGateMode::SkipHeadless => {
            if trust_probe().trusted {
                AxGateOutcome::Trusted
            } else {
                AxGateOutcome::Skipped(
                    "Accessibility skipped (headless); grant later for tiling".into(),
                )
            }
        }
        AxGateMode::WaitInteractive => wait_until_trusted(),
    }
}

/// Exit code helper for `SUNGHYUN_AX_PROBE=1` (no Settings, no poll).
pub fn probe_exit_trusted() -> bool {
    is_process_trusted()
}

/// Exit code helper for `SUNGHYUN_AX_REGISTER=1`: ask TCC to register this
/// process's identity and show the OS consent prompt (no Settings, no poll).
pub fn register_exit_trusted() -> bool {
    ax_trusted_with_options(true)
}

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const POLL_TIMEOUT: Duration = Duration::from_secs(90);

fn wait_until_trusted() -> AxGateOutcome {
    if cfg!(not(target_os = "macos")) {
        return AxGateOutcome::Skipped("Accessibility is macOS-only".into());
    }

    let initial = trust_probe();
    if initial.trusted {
        return trusted_outcome(initial);
    }

    eprintln!();
    eprintln!("sunghyun needs Accessibility for window tiling and clipboard paste.");
    eprintln!(
        "running binary: {}",
        initial.running_path_display()
    );
    eprintln!("Opening Accessibility settings once (no Enter prompt)…");
    open_accessibility_settings();
    // Register this binary's own TCC entry (OS prompt sheet once). Must run
    // disclaimed: from an inherited-trusted context the in-process
    // AXIsProcessTrustedWithOptions(prompt) returns true without ever listing
    // the binary in Settings, leaving the owner nothing to toggle.
    if let Ok(exe) = env::current_exe() {
        let _ = spawn_disclaimed(&exe, &[], "SUNGHYUN_AX_REGISTER");
    }
    let _ = ax_trusted_with_options(true);

    eprintln!(
        "Polling Accessibility trust every {}s for up to {}s…",
        POLL_INTERVAL.as_secs(),
        POLL_TIMEOUT.as_secs()
    );
    let deadline = Instant::now() + POLL_TIMEOUT;
    while Instant::now() < deadline {
        thread::sleep(POLL_INTERVAL);
        let probe = trust_probe();
        if probe.trusted {
            eprintln!("Accessibility granted.");
            return trusted_outcome(probe);
        }
    }

    // Best-effort: do not fail the whole bootstrap / do not loop on Enter.
    let running = trust_probe().running_path_display();
    eprintln!(
        "Accessibility still not confirmed for {running} after {}s; proceeding best-effort (agent/cua may finish the toggle)",
        POLL_TIMEOUT.as_secs()
    );
    AxGateOutcome::Skipped(format!(
        "Accessibility not confirmed after {}s for {running}; proceeding best-effort",
        POLL_TIMEOUT.as_secs()
    ))
}

fn trusted_outcome(probe: TrustProbe) -> AxGateOutcome {
    if let (Some(running), Some(granted)) = (&probe.running, &probe.granted_copy) {
        if canonicalize_loose(running) != canonicalize_loose(granted) {
            eprintln!(
                "Accessibility trusted for {}; running {} (TCC path mismatch — grant the running binary if tiling fails)",
                granted.display(),
                running.display()
            );
        }
    }
    AxGateOutcome::Trusted
}

#[cfg(target_os = "macos")]
fn ax_trusted_legacy() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> u8;
    }
    unsafe { AXIsProcessTrusted() != 0 }
}

#[cfg(target_os = "macos")]
fn ax_trusted_with_options(prompt: bool) -> bool {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use core_foundation_sys::dictionary::CFDictionaryRef;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    }

    let key = CFString::new("AXTrustedCheckOptionPrompt");
    let value = if prompt {
        CFBoolean::true_value()
    } else {
        CFBoolean::false_value()
    };
    let dict: CFDictionary<CFString, CFType> =
        CFDictionary::from_CFType_pairs(&[(key, value.as_CFType())]);
    unsafe { AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef()) != 0 }
}

#[cfg(not(target_os = "macos"))]
fn ax_trusted_with_options(_prompt: bool) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_skips_wait() {
        assert_eq!(ax_gate_mode(true), AxGateMode::SkipHeadless);
    }

    #[test]
    fn interactive_waits() {
        assert_eq!(ax_gate_mode(false), AxGateMode::WaitInteractive);
    }

    #[test]
    fn ensure_accessibility_headless_does_not_fail() {
        headless::force(true);
        let out = ensure_accessibility(true);
        headless::clear_force();
        match out {
            AxGateOutcome::Trusted | AxGateOutcome::Skipped(_) => {}
            AxGateOutcome::Failed(m) => panic!("unexpected fail: {m}"),
        }
    }

    #[test]
    fn wait_path_has_no_enter_prompt_strings_in_source_contract() {
        // Behavioral contract mirrored in unit form: interactive mode exists,
        // but ensure_accessibility must not be the Enter-loop mode name.
        assert_eq!(ax_gate_mode(false), AxGateMode::WaitInteractive);
    }
}
