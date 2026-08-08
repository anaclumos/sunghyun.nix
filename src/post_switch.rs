//! Residual steps after `darwin-rebuild switch`.
//!
//! Nix owns packages/daemons/files. This command covers only OS-native
//! one-time surfaces: DriverKit dext approval, Accessibility, keyboard-engine
//! first launch (each = open the pane / let macOS prompt, poll, skip-not-fail)
//! plus the two live-state restores (Spotlight hotkey patch, menu bar).

use crate::assets;
use crate::bootstrap::load_embedded_manifest;
use crate::bootstrap::steps::{self, StepContext, StepOutcome};
use crate::bootstrap::sudo_keepalive::SudoKeepAlive;
use crate::bootstrap::BootstrapManifest;
use crate::headless;
use crate::status::{Report, StepReport};
use std::path::PathBuf;

pub struct PostSwitchOpts {
    pub dry_run: bool,
    pub headless: bool,
    pub manifest: BootstrapManifest,
}

fn outcome_to_report(id: &str, outcome: StepOutcome) -> StepReport {
    match outcome {
        StepOutcome::Ok(msg) => StepReport::ok(id, msg),
        StepOutcome::Skipped(msg) => StepReport::skipped(id, msg),
        StepOutcome::Failed(msg) => StepReport::failed(id, msg),
    }
}

/// Residual steps (no brew/mas/omz; those belong to nix-darwin activation).
fn gate_table() -> Vec<(&'static str, fn(&StepContext) -> StepOutcome)> {
    vec![
        ("karabiner_driverkit", steps::step_karabiner_driverkit),
        ("keyboard_engine", steps::step_keyboard_engine),
        ("accessibility", steps::step_accessibility),
        ("default_browser", steps::step_default_browser),
        ("spotlight", steps::step_spotlight),
        ("menubar", steps::step_menubar),
        ("cua_driver", steps::step_cua_driver),
    ]
}

pub fn run(opts: &PostSwitchOpts) -> Report {
    if opts.headless {
        headless::force(true);
    }
    let headless_mode = headless::is_headless();

    let home = directories::UserDirs::new()
        .map(|u| u.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"));

    let mut steps = Vec::new();
    steps.push(StepReport::ok(
        "post_switch",
        if headless_mode {
            "headless post-switch (GUI gates skip)".to_string()
        } else {
            "interactive post-switch after darwin-rebuild".to_string()
        },
    ));

    let ctx_base = StepContext {
        dry_run: opts.dry_run,
        headless: headless_mode,
        manifest: opts.manifest.clone(),
        home: home.clone(),
        state_dir: assets::config_dir(&home),
    };

    // App Store sign-in: no gate at all (owner policy 2026-08-07). Setup
    // Assistant handles it; mas apps skip gracefully in nix activation when
    // signed out and converge on a later switch.

    let _sudo = if opts.dry_run || headless_mode {
        SudoKeepAlive::noop()
    } else {
        SudoKeepAlive::acquire()
    };

    for (id, f) in gate_table() {
        if ctx_base.manifest.skip.iter().any(|s| s == id) {
            steps.push(StepReport::skipped(id, "skipped by manifest"));
            continue;
        }
        let outcome = f(&ctx_base);
        steps.push(outcome_to_report(id, outcome));
    }

    Report {
        headless: ctx_base.headless,
        steps,
    }
}

pub fn default_manifest() -> BootstrapManifest {
    load_embedded_manifest()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_table_excludes_brew() {
        let ids: Vec<_> = gate_table().into_iter().map(|(id, _)| id).collect();
        assert!(ids.contains(&"accessibility"));
        assert!(ids.contains(&"spotlight"));
        assert!(ids.contains(&"menubar"));
        assert!(!ids.contains(&"homebrew"));
        assert!(!ids.contains(&"brew_bundle"));
        assert!(!ids.contains(&"mas"));
    }
}
