use std::fs::File;
use std::io::{self, IsTerminal};
use std::process::{Child, Command, Stdio};

/// Interactive `sudo -v` once, then a background refresher so brew/pkg/LaunchDaemon
/// steps do not re-prompt. Drop kills the keeper.
pub struct SudoKeepAlive {
    keeper: Option<Child>,
}

impl SudoKeepAlive {
    /// No-op (dry-run / skipped privilege section).
    pub fn noop() -> Self {
        Self { keeper: None }
    }

    /// Cache sudo credentials for the rest of bootstrap.
    ///
    /// - If `sudo -n true` already works, start the keeper without prompting.
    /// - Else, when a terminal is reachable, run interactive `sudo -v` once.
    /// - Non-interactive without a cache: warn and continue (`run_root` may
    ///   still prompt on /dev/tty later; owner allows rare prompts).
    pub fn acquire() -> Self {
        if sudo_n_true() {
            return Self {
                keeper: spawn_keeper(),
            };
        }

        if !io::stdin().is_terminal() && !dev_tty_available() {
            eprintln!(
                "sudo: no credential cache and no TTY; privileged steps will fail if they need root"
            );
            return Self::noop();
        }

        eprintln!("sudo: enter password once for privileged steps");
        let ok = Command::new("sudo")
            .arg("-v")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            eprintln!("sudo -v failed; continuing without privilege cache");
            return Self::noop();
        }

        // Root-cause instrumentation: on a stock macOS sudo the ticket from
        // `sudo -v` must satisfy an immediate child `sudo -n` on the same tty.
        // If this misses, the host has a non-caching policy (timestamp_timeout=0
        // or tty/session mismatch) and `run_root` will use its interactive
        // fallback instead of hard-failing.
        if !sudo_n_true() {
            eprintln!(
                "sudo: ticket from sudo -v not visible to child sudo -n (non-caching sudoers policy?); \
                 privileged steps fall back to interactive sudo"
            );
        }

        Self {
            keeper: spawn_keeper(),
        }
    }
}

impl Drop for SudoKeepAlive {
    fn drop(&mut self) {
        if let Some(mut child) = self.keeper.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn sudo_n_true() -> bool {
    Command::new("sudo")
        .args(["-n", "true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn dev_tty_available() -> bool {
    File::open("/dev/tty").is_ok()
}

fn spawn_keeper() -> Option<Child> {
    let main_pid = std::process::id();
    // Classic keep-alive: refresh the sudo timestamp until the parent exits.
    // `|| true` keeps the loop alive when -n misses (tty_tickets / no TTY).
    let script = format!(
        "while true; do sudo -n true 2>/dev/null || true; sleep 60; kill -0 {main_pid} || exit; done"
    );
    Command::new("/bin/sh")
        .args(["-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()
}

/// Run a privileged command with `sudo -n` (fail fast if the cache expired).
/// Probe-style callers only; command paths should prefer [`run_root`].
pub fn run_sudo_n(args: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new("sudo");
    cmd.arg("-n").args(args).stdin(Stdio::null());
    let output = cmd
        .output()
        .map_err(|e| format!("spawn sudo -n: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "sudo -n {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn sudo_n_auth_missing(stderr: &str) -> bool {
    stderr.contains("password is required")
}

/// Run a privileged command, never failing solely because `sudo -n` missed.
///
/// 1. `sudo -n` first (silent when the keep-alive ticket works).
/// 2. If — and only if — `-n` failed for lack of a cached credential and a
///    terminal is reachable, retry with plain interactive `sudo` so the ticket
///    or one owner-typed password satisfies it (owner allows rare prompts;
///    never osascript admin / SecurityAgent spam).
pub fn run_root(args: &[&str]) -> Result<(), String> {
    if args.is_empty() {
        return Err("run_root: empty argv".into());
    }
    let mut cmd = Command::new("sudo");
    cmd.arg("-n").args(args).stdin(Stdio::null());
    let output = cmd
        .output()
        .map_err(|e| format!("spawn sudo -n: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !sudo_n_auth_missing(&stderr) {
        return Err(format!("sudo -n {args:?} failed: {stderr}"));
    }
    if !dev_tty_available() {
        return Err(format!(
            "sudo {args:?} needs a credential but no terminal is reachable: {stderr}"
        ));
    }
    eprintln!("sudo: cached ticket unavailable; prompting once for {:?}", args[0]);
    let status = Command::new("sudo")
        .args(args)
        .status()
        .map_err(|e| format!("spawn sudo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("sudo {args:?} exited {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_drops_cleanly() {
        let k = SudoKeepAlive::noop();
        drop(k);
    }

    #[test]
    fn auth_missing_detection() {
        assert!(sudo_n_auth_missing("sudo: a password is required\n"));
        assert!(!sudo_n_auth_missing("rm: /x: No such file or directory\n"));
    }

    #[test]
    fn run_root_rejects_empty() {
        assert!(run_root(&[]).is_err());
    }
}
