//! Residual steps for `sunghyun post-switch`.
//!
//! nix-darwin owns packages, files, defaults, and launchd. The only human
//! surface left (owner policy 2026-08-07) is macOS's own one-time prompts:
//! open the exact Settings pane / let the OS prompt, poll for the grant with
//! a sane timeout, proceed when granted, and degrade gracefully (skip, not
//! fail) on timeout — the system converges on a later switch. No agents, no
//! instruction text, no stdin prompts.

use super::sudo_keepalive::run_root;
use super::BootstrapManifest;
use crate::ax::{self, AxGateOutcome};
use crate::default_browser;
use crate::error::ActionError;
use crate::headless;
use crate::kanata_ctl;
use crate::menubar;
use crate::spotlight;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct StepContext {
    pub dry_run: bool,
    pub headless: bool,
    pub manifest: BootstrapManifest,
    pub home: PathBuf,
    pub state_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    Ok(String),
    Skipped(String),
    Failed(String),
}

pub fn command_exists(name: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {name} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_cmd(program: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let output = cmd
        .output()
        .map_err(|e| format!("spawn {}: {e}", program.display()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{} {:?} failed: {}",
            program.display(),
            args,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn open_url(url: &str) -> bool {
    Command::new("open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Open a Settings pane once, then poll `probe` until it passes or the budget
/// runs out. Timeout is a graceful skip (converge on next switch), never a
/// failure. The owner clicking the toggle in the opened window is the entire
/// human surface.
fn open_and_poll(
    what: &str,
    pane_url: &str,
    budget: Duration,
    probe: &dyn Fn() -> bool,
) -> StepOutcome {
    if probe() {
        return StepOutcome::Ok(format!("{what} already granted"));
    }
    if headless::is_headless() {
        return StepOutcome::Skipped(format!("{what} skipped (headless); converges later"));
    }
    eprintln!("{what}: opening System Settings pane; waiting for the toggle (no prompts)…");
    open_url(pane_url);
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_secs(3));
        if probe() {
            return StepOutcome::Ok(format!("{what} granted"));
        }
    }
    StepOutcome::Skipped(format!(
        "{what} not granted within {}s; skipping (the next darwin-rebuild switch reopens this pane on its own)",
        budget.as_secs()
    ))
}

pub fn step_cua_driver(ctx: &StepContext) -> StepOutcome {
    if ctx.headless {
        return StepOutcome::Skipped("CuaDriver skipped (headless)".into());
    }
    if command_exists("cua-driver") || ctx.home.join(".local/bin/cua-driver").exists() {
        return StepOutcome::Ok("cua-driver present (dev/debug tool)".into());
    }
    // cua-driver is a dev/debug tool in the toolbox, not a product dependency
    // (owner policy 2026-08-07). Never install it from the product path.
    StepOutcome::Skipped("cua-driver absent (dev tool; not a product dependency)".into())
}

pub fn step_karabiner_driverkit(ctx: &StepContext) -> StepOutcome {
    if cfg!(not(target_os = "macos")) {
        return StepOutcome::Skipped("DriverKit is macOS-only".into());
    }
    if ctx.headless {
        return StepOutcome::Skipped("Karabiner-DriverKit skipped (headless)".into());
    }
    let installed = Path::new(
        "/Applications/.Karabiner-VirtualHIDDevice-Manager.app/Contents/MacOS/Karabiner-VirtualHIDDevice-Manager",
    )
    .exists()
        || Path::new("/Library/Application Support/org.pqrs/Karabiner-DriverKit-VirtualHIDDevice")
            .exists()
        || Path::new("/Applications/Karabiner-Elements.app").exists();
    if !installed {
        if ctx.dry_run {
            return StepOutcome::Skipped(
                "would install Karabiner-DriverKit-VirtualHIDDevice v6.2.0 pkg".into(),
            );
        }
        // Standalone pqrs pkg, never via cask cleanup paths.
        let url = ctx.manifest.kanata_driver_url.as_deref().unwrap_or(
            "https://github.com/pqrs-org/Karabiner-DriverKit-VirtualHIDDevice/releases/download/v6.2.0/Karabiner-DriverKit-VirtualHIDDevice-6.2.0.pkg",
        );
        let pkg = ctx
            .state_dir
            .join("Karabiner-DriverKit-VirtualHIDDevice-6.2.0.pkg");
        let Some(pkg_str) = pkg.to_str() else {
            return StepOutcome::Failed("DriverKit pkg path is not UTF-8".into());
        };
        if let Err(e) = fs::create_dir_all(&ctx.state_dir) {
            return StepOutcome::Failed(format!("DriverKit state dir: {e}"));
        }
        eprintln!("Downloading Karabiner-DriverKit v6.2.0…");
        if let Err(e) = run_cmd(
            Path::new("/usr/bin/curl"),
            &["-fsSL", "-o", pkg_str, url],
            &[],
        ) {
            return StepOutcome::Failed(format!("DriverKit download failed: {e}"));
        }
        if let Err(e) = run_root(&["installer", "-pkg", pkg_str, "-target", "/"]) {
            return StepOutcome::Failed(format!("DriverKit installer: {e}"));
        }
    }
    if ctx.dry_run {
        return StepOutcome::Skipped("would poll dext approval".into());
    }
    // dext approval has no declarative/CLI path (systemextensionsctl has no
    // approve verb; PPPC/sysext policy is MDM-only): open the pane and poll.
    open_and_poll(
        "DriverKit dext approval",
        "x-apple.systempreferences:com.apple.LoginItems-Settings.extension",
        Duration::from_secs(120),
        &kanata_ctl::vhid_dext_activated,
    )
}

pub fn step_accessibility(ctx: &StepContext) -> StepOutcome {
    if ctx.dry_run {
        return StepOutcome::Skipped("would probe Accessibility and open pane if missing".into());
    }
    if ctx.headless {
        return StepOutcome::Skipped("Accessibility skipped (headless)".into());
    }
    let probe = || matches!(ax::accessibility_status(), AxGateOutcome::Trusted);
    open_and_poll(
        "Accessibility (sunghyun tiling)",
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        Duration::from_secs(120),
        &probe,
    )
}

/// Primary keyboard engine (OUTCOMES.md a-e): Karabiner-Elements. Launching it
/// once triggers macOS's own permission prompts (background items, Input
/// Monitoring/Accessibility for the grabber); we poll for the grabber being
/// alive and skip gracefully on timeout.
pub fn step_keyboard_engine(ctx: &StepContext) -> StepOutcome {
    if cfg!(not(target_os = "macos")) {
        return StepOutcome::Skipped("keyboard engine is macOS-only".into());
    }
    if ctx.headless {
        return StepOutcome::Skipped("keyboard engine skipped (headless)".into());
    }
    if !Path::new("/Applications/Karabiner-Elements.app").exists() {
        return StepOutcome::Skipped(
            "Karabiner-Elements not installed yet (homebrew cask installs it on switch)".into(),
        );
    }
    // KE >= 15 renamed karabiner_grabber to Karabiner-Core-Service.
    let grabber_up = || {
        Command::new("pgrep")
            .args(["-f", "Karabiner-Core-Service|karabiner_grabber"])
            .stdout(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    if grabber_up() {
        return StepOutcome::Ok("Karabiner-Elements grabber running".into());
    }
    if ctx.dry_run {
        return StepOutcome::Skipped("would launch Karabiner-Elements once for OS prompts".into());
    }
    eprintln!("keyboard engine: launching Karabiner-Elements once (macOS shows its own permission prompts)…");
    let _ = Command::new("open")
        .args(["-a", "Karabiner-Elements"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let deadline = Instant::now() + Duration::from_secs(120);
    while Instant::now() < deadline {
        thread::sleep(Duration::from_secs(3));
        if grabber_up() {
            return StepOutcome::Ok("Karabiner-Elements grabber running (grants accepted)".into());
        }
    }
    StepOutcome::Skipped(
        "Karabiner-Elements grabber not up within 120s; approve the OS prompts, it converges automatically"
            .into(),
    )
}

/// Dia as the default browser. macOS owns the confirmation panel for this
/// change and there is no declarative path to it, so this behaves like the TCC
/// gates: trigger the system's own panel, bring it forward, poll, and treat an
/// unanswered panel as a skip that the next switch retries.
pub fn step_default_browser(ctx: &StepContext) -> StepOutcome {
    if cfg!(not(target_os = "macos")) {
        return StepOutcome::Skipped("default browser step is macOS-only".into());
    }
    if ctx.headless || headless::is_headless() {
        return StepOutcome::Skipped("default browser skipped (headless)".into());
    }
    if ctx.dry_run {
        return StepOutcome::Skipped("would ask macOS to make Dia the default browser".into());
    }
    match default_browser::converge(default_browser::DIA_BUNDLE_ID, Duration::from_secs(120)) {
        Ok(msg) => StepOutcome::Ok(msg),
        Err(ActionError::Skipped(m)) => StepOutcome::Skipped(m),
        Err(e) => StepOutcome::Failed(e.to_string()),
    }
}

/// Spotlight ⌘Space (symbolichotkeys id 64) stays imperative on purpose:
/// `defaults write` (and nix-darwin CustomUserPreferences) can only replace the
/// whole AppleSymbolicHotKeys dict, which would clobber every other shortcut
/// (native option PR nix-darwin#1741 unmerged as of 2026-08-07). This step
/// reads the live plist and patches only entry 64.
pub fn step_spotlight(ctx: &StepContext) -> StepOutcome {
    if ctx.headless || headless::is_headless() {
        return StepOutcome::Skipped("Spotlight restore skipped (headless)".into());
    }
    if cfg!(not(target_os = "macos")) {
        return StepOutcome::Skipped("Spotlight is macOS-only".into());
    }
    if ctx.dry_run {
        return StepOutcome::Skipped(
            "would restore Spotlight ⌘Space, Clipboard History, and the terminal alias".into(),
        );
    }

    // Converge all three Spotlight outcomes. An early return on "⌘Space is
    // already enabled" is wrong: that is the factory default, so on a fresh Mac
    // the step used to exit before ever installing ~/Applications/terminal.app,
    // and `verify` closed every first install with
    //   [failed] terminal_alias: ... run `sunghyun spotlight restore`
    // which is both a false failure and an owner instruction the CLI can do
    // itself.
    let mut done: Vec<&str> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut failed: Vec<String> = Vec::new();

    match spotlight::is_command_space_enabled() {
        Ok(true) => done.push("⌘Space"),
        Ok(false) => match spotlight::restore_command_space() {
            Ok(()) => done.push("⌘Space restored"),
            Err(crate::error::ActionError::Skipped(m)) => skipped.push(m),
            Err(e) => failed.push(e.to_string()),
        },
        Err(crate::error::ActionError::Skipped(m)) => skipped.push(m),
        Err(e) => failed.push(e.to_string()),
    }

    match spotlight::enable_pasteboard_history() {
        Ok(()) => done.push("Clipboard History"),
        Err(crate::error::ActionError::Skipped(m)) => skipped.push(m),
        Err(e) => failed.push(e.to_string()),
    }

    match spotlight::install_terminal_ghostty_alias(&ctx.home) {
        Ok(()) => done.push("terminal→Ghostty alias"),
        Err(crate::error::ActionError::Skipped(m)) => skipped.push(m),
        Err(e) => failed.push(e.to_string()),
    }

    if !failed.is_empty() {
        return StepOutcome::Failed(failed.join("; "));
    }
    if done.is_empty() {
        return StepOutcome::Skipped(skipped.join("; "));
    }
    let mut msg = done.join(", ");
    if !skipped.is_empty() {
        msg.push_str(&format!(" (skipped: {})", skipped.join("; ")));
    }
    StepOutcome::Ok(msg)
}

/// Time Machine menu extra is declared in nix-darwin CustomUserPreferences and
/// only re-checked here; Cursor tray hiding is app storage (not defaults).
pub fn step_menubar(ctx: &StepContext) -> StepOutcome {
    if ctx.headless || headless::is_headless() {
        return StepOutcome::Skipped("Menu bar restore skipped (headless)".into());
    }
    if cfg!(not(target_os = "macos")) {
        return StepOutcome::Skipped("Menu bar restore is macOS-only".into());
    }

    let tm_ok = match menubar::is_time_machine_hidden() {
        Ok(v) => v,
        Err(crate::error::ActionError::Skipped(m)) => return StepOutcome::Skipped(m),
        Err(e) => return StepOutcome::Failed(e.to_string()),
    };
    let cursor_ok = match menubar::is_cursor_tray_hidden(&ctx.home) {
        Ok(v) => v,
        Err(crate::error::ActionError::Skipped(m)) => return StepOutcome::Skipped(m),
        Err(e) => return StepOutcome::Failed(e.to_string()),
    };
    if tm_ok && cursor_ok {
        return StepOutcome::Ok("Time Machine + Cursor already hidden from menu bar".into());
    }
    if ctx.dry_run {
        return StepOutcome::Skipped("would hide Time Machine + Cursor menu bar extras".into());
    }

    let mut parts: Vec<String> = Vec::new();
    if !tm_ok {
        match menubar::hide_time_machine() {
            Ok(()) => parts.push("Time Machine hidden".into()),
            Err(crate::error::ActionError::Skipped(m)) => parts.push(m),
            Err(e) => return StepOutcome::Failed(e.to_string()),
        }
    } else {
        parts.push("Time Machine already hidden".into());
    }
    if !cursor_ok {
        match menubar::hide_cursor_tray(&ctx.home) {
            Ok(()) => parts.push("Cursor tray hidden (restart Cursor if still visible)".into()),
            Err(crate::error::ActionError::Skipped(m)) => {
                return StepOutcome::Ok(format!("{}; Cursor skipped: {m}", parts.join("; ")))
            }
            Err(e) => return StepOutcome::Failed(e.to_string()),
        }
    } else {
        parts.push("Cursor tray already hidden".into());
    }
    StepOutcome::Ok(parts.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_steps_skip_not_fail() {
        let ctx = StepContext {
            dry_run: false,
            headless: true,
            manifest: BootstrapManifest::default(),
            home: PathBuf::from("/tmp"),
            state_dir: PathBuf::from("/tmp/x"),
        };
        for (name, out) in [
            ("driverkit", step_karabiner_driverkit(&ctx)),
            ("accessibility", step_accessibility(&ctx)),
            ("keyboard_engine", step_keyboard_engine(&ctx)),
            ("cua_driver", step_cua_driver(&ctx)),
        ] {
            match out {
                StepOutcome::Skipped(_) | StepOutcome::Ok(_) => {}
                StepOutcome::Failed(m) => panic!("{name} must not fail headless: {m}"),
            }
        }
    }

    #[test]
    fn no_agent_or_instruction_machinery_in_this_module() {
        let src = include_str!("steps.rs");
        assert!(
            !src.contains(concat!("agent", " -p")),
            "no GUI agent spawning in product steps"
        );
        assert!(
            !src.contains(concat!("Press", " Enter")),
            "no stdin prompts"
        );
    }

    #[test]
    fn command_probe_does_not_panic() {
        let _ = command_exists("git");
    }
}
