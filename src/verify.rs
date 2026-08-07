use crate::actions::tile::TileAction;
use crate::assets;
use crate::ax::{self, AxGateOutcome};
use crate::config::{load_or_default, Config};
use crate::headless;
use crate::menubar;
use crate::spotlight;
use crate::status::{Report, StepReport};
use std::process::Command;

pub struct VerifyOpts {
    pub config_path: Option<std::path::PathBuf>,
    #[allow(dead_code)]
    pub json: bool,
    pub headless: bool,
}

pub fn run(opts: &VerifyOpts) -> Report {
    if opts.headless {
        headless::force(true);
    }
    let headless_mode = headless::is_headless();
    let mut steps = Vec::new();

    let (config, loaded_from) = match load_or_default(opts.config_path.as_deref()) {
        Ok(v) => v,
        Err(e) => {
            steps.push(StepReport::failed("config", e.to_string()));
            return Report {
                headless: headless_mode,
                steps,
            };
        }
    };
    steps.push(match &loaded_from {
        Some(p) => StepReport::ok("config", format!("loaded {}", p.display())),
        None => StepReport::ok("config", "using built-in defaults"),
    });

    steps.push(check_binary_features());
    steps.push(check_ime_mapping(&config));
    steps.push(check_apps(&config));
    steps.push(check_tiles());
    steps.push(check_clipboard(&config));
    steps.push(check_launcher(&config));
    steps.push(check_keyboard_engine());
    steps.push(check_kanata_config(&config));
    steps.push(check_hushlogin());
    steps.push(check_spotlight());
    steps.push(check_spotlight_clipboard());
    steps.push(check_terminal_alias());
    steps.push(check_menubar());
    steps.push(check_ax_permission(headless_mode));
    steps.push(check_input_monitoring());

    Report {
        headless: headless_mode,
        steps,
    }
}

fn check_binary_features() -> StepReport {
    StepReport::ok(
        "binary",
        "features: open,input-source,tile,launcher,clipboard,verify,post-switch,kanata",
    )
}

/// Outcome check (OUTCOMES.md a-e): a tap-hold keyboard engine is configured
/// with the sunghyun binding set. Primary engine today: Karabiner-Elements
/// declarative JSON. Asserts outcome tokens, not engine internals, so the
/// engine can be swapped without touching this check.
fn check_keyboard_engine() -> StepReport {
    let Some(home) = directories::UserDirs::new().map(|u| u.home_dir().to_path_buf()) else {
        return StepReport::failed("keyboard_engine", "no home directory");
    };
    let karabiner = home.join(".config/karabiner/karabiner.json");
    match std::fs::read_to_string(&karabiner) {
        Ok(text) => {
            let outcomes = [
                ("caps tap = maximize", "tile maximize"),
                ("hyper tiling", "tile left"),
                ("hyper browser", "open-default-browser"),
                ("cmd tap = IME", "input-source ABC"),
                // ⌘⇧V sends virtual ⌘Space then ⌘4 (spacebar only appears in
                // that rule); the shell_command CLI hop was removed 2026-08-08.
                ("cmd-shift-v clipboard", "spacebar"),
            ];
            let missing: Vec<&str> = outcomes
                .iter()
                .filter(|(_, token)| !text.contains(token))
                .map(|(name, _)| *name)
                .collect();
            if missing.is_empty() {
                StepReport::ok(
                    "keyboard_engine",
                    format!("karabiner.json covers outcomes a-e ({})", karabiner.display()),
                )
            } else {
                StepReport::failed(
                    "keyboard_engine",
                    format!("karabiner.json missing outcomes: {}", missing.join(", ")),
                )
            }
        }
        Err(_) => StepReport::skipped(
            "keyboard_engine",
            "no karabiner.json yet (darwin-rebuild switch materializes it); kanata remains the opt-in alternative",
        ),
    }
}

fn check_ime_mapping(config: &Config) -> StepReport {
    let abc = config.resolve_ime_id("ABC");
    let ko = config.resolve_ime_id("2SetKorean");
    match (abc, ko) {
        (Some(a), Some(k)) => StepReport::ok("ime_map", format!("ABC={a} korean={k}")),
        _ => StepReport::failed("ime_map", "IME id mapping incomplete"),
    }
}

fn check_apps(config: &Config) -> StepReport {
    // Hyper+J uses `open browser` (OS default HTTP handler), not a fixed apps.* key.
    let required = ["calendar", "mail", "slack", "ghostty"];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|k| config.resolve_app(k).is_none())
        .collect();
    if missing.is_empty() {
        StepReport::ok("apps", format!("{} app keys resolvable", required.len()))
    } else {
        StepReport::failed("apps", format!("missing keys: {missing:?}"))
    }
}

fn check_tiles() -> StepReport {
    let all = [
        "left",
        "right",
        "top",
        "bottom",
        "center",
        "top-left",
        "first-fourth",
        "second-fourth",
        "third-fourth",
        "last-fourth",
        "last-three-fourths",
        "maximize",
        "right-third",
        "fullscreen",
    ];
    let ok = all.iter().all(|n| TileAction::parse(n).is_some());
    if ok {
        StepReport::ok("tiles", format!("{} tile actions mapped", all.len()))
    } else {
        StepReport::failed("tiles", "tile action parse incomplete")
    }
}

fn check_clipboard(config: &Config) -> StepReport {
    if !config.clipboard.enabled {
        return StepReport::skipped(
            "clipboard",
            "sunghyun clipboard picker disabled; ⌘⇧V opens Spotlight Clipboard (karabiner sends ⌘Space then ⌘4)",
        );
    }
    if headless::is_headless() {
        return StepReport::ok(
            "clipboard",
            format!(
                "history_limit={} (GUI picker skipped in headless)",
                config.clipboard.history_limit
            ),
        );
    }
    StepReport::ok(
        "clipboard",
        format!("history_limit={}", config.clipboard.history_limit),
    )
}

fn check_launcher(config: &Config) -> StepReport {
    if !config.launcher.enabled {
        return StepReport::skipped("launcher", "disabled (Spotlight owns ⌘Space by default)");
    }
    StepReport::ok(
        "launcher",
        "optional launcher available; default Mac uses Spotlight ⌘Space",
    )
}

fn check_kanata_config(config: &Config) -> StepReport {
    let mut candidates = Vec::new();
    if let Some(p) = config.paths.kanata_kbd.as_ref() {
        let expanded = match p.to_str() {
            Some(s) if s.starts_with("~/") => {
                directories::UserDirs::new().map(|u| u.home_dir().join(&s[2..]))
            }
            _ => Some(p.clone()),
        };
        if let Some(p) = expanded {
            candidates.push(p);
        }
    }
    if let Some(home) = directories::UserDirs::new() {
        candidates.push(home.home_dir().join(".config/sunghyun/kanata.kbd"));
    }
    for c in candidates {
        if c.is_file() {
            let raw = std::fs::read_to_string(&c).unwrap_or_default();
            if raw.contains("clipboard show") {
                return StepReport::failed(
                    "kanata_kbd",
                    format!(
                        "{} still binds the clipboard picker; ⌘⇧V (native macro) owns clipboard",
                        c.display()
                    ),
                );
            }
            if !(raw.contains("@lcmd") && raw.contains("@rcmd")) {
                return StepReport::failed(
                    "kanata_kbd",
                    format!(
                        "{} missing dual-function ⌘ (@lcmd/@rcmd tap=IME hold=⌘)",
                        c.display()
                    ),
                );
            }
            if !(raw.contains("lmet") && raw.contains("rmet")) {
                return StepReport::failed(
                    "kanata_kbd",
                    format!("{} missing lmet/rmet hold targets", c.display()),
                );
            }
            if !raw.contains("tile maximize") {
                return StepReport::failed(
                    "kanata_kbd",
                    format!("{} missing Caps-tap tile maximize", c.display()),
                );
            }
            // ⌘⇧V must be a native key macro (M-spc … M-4); a `spotlight
            // clipboard` CLI hop is dead on macOS 26+ (WindowServer drops
            // synthesized keys before the hotkey matcher).
            if raw.contains("spotlight clipboard") || !raw.contains("M-spc") {
                return StepReport::failed(
                    "kanata_kbd",
                    format!(
                        "{} must bind ⌘⇧V as a native macro (M-spc then M-4), not a CLI hop",
                        c.display()
                    ),
                );
            }
            return StepReport::ok(
                "kanata_kbd",
                format!(
                    "found {} (⌘ tap=IME hold=mod; Caps maximize; ⌘⇧V Spotlight clipboard)",
                    c.display()
                ),
            );
        }
    }
    if headless::is_headless() {
        StepReport::skipped(
            "kanata_kbd",
            "kanata.kbd not found under ~/.config/sunghyun (ok to provision later)",
        )
    } else {
        StepReport::failed(
            "kanata_kbd",
            format!(
                "kanata.kbd not found (install.sh / enable --safe materializes shipped asset, {} bytes)",
                assets::KANATA_KBD.len()
            ),
        )
    }
}

fn check_hushlogin() -> StepReport {
    let Some(home) = directories::UserDirs::new() else {
        return StepReport::skipped("hushlogin", "no home directory");
    };
    let path = home.home_dir().join(".hushlogin");
    if path.is_file() {
        StepReport::ok("hushlogin", format!("{} present", path.display()))
    } else if headless::is_headless() {
        StepReport::skipped(
            "hushlogin",
            "~/.hushlogin missing (ok in headless; bootstrap creates it)",
        )
    } else {
        StepReport::failed(
            "hushlogin",
            "~/.hushlogin missing; darwin-rebuild/home-manager switch materializes it",
        )
    }
}

fn check_spotlight() -> StepReport {
    match spotlight::is_command_space_enabled() {
        Ok(true) => StepReport::ok("spotlight", "⌘Space Show Spotlight search enabled"),
        Ok(false) => StepReport::failed(
            "spotlight",
            "Spotlight ⌘Space disabled; run `sunghyun spotlight restore` or enable in System Settings",
        ),
        Err(crate::error::ActionError::Skipped(m)) => StepReport::skipped("spotlight", m),
        Err(e) => StepReport::failed("spotlight", e.to_string()),
    }
}

fn check_menubar() -> StepReport {
    let Some(home) = directories::UserDirs::new() else {
        return StepReport::skipped("menubar", "no home directory");
    };
    let tm = menubar::is_time_machine_hidden();
    let cursor = menubar::is_cursor_tray_hidden(home.home_dir());
    match (tm, cursor) {
        (Ok(true), Ok(true)) => StepReport::ok(
            "menubar",
            "Time Machine + Cursor hidden from menu bar",
        ),
        (Ok(tm_ok), Ok(cursor_ok)) => StepReport::failed(
            "menubar",
            format!(
                "menu bar extras still visible (Time Machine hidden={tm_ok}, Cursor tray hidden={cursor_ok}); run `sunghyun post-switch` (menubar step)"
            ),
        ),
        (Err(crate::error::ActionError::Skipped(m)), _)
        | (_, Err(crate::error::ActionError::Skipped(m))) => StepReport::skipped("menubar", m),
        (Err(e), _) | (_, Err(e)) => StepReport::failed("menubar", e.to_string()),
    }
}

fn check_spotlight_clipboard() -> StepReport {
    match spotlight::is_pasteboard_history_enabled() {
        Ok(true) => StepReport::ok(
            "spotlight_clipboard",
            "Clipboard History on; ⌘⇧V sends ⌘Space then ⌘4 via Karabiner (Apple has no native global hotkey)",
        ),
        Ok(false) => StepReport::failed(
            "spotlight_clipboard",
            "PasteboardHistoryEnabled off; run `sunghyun spotlight restore` or enable Clipboard History in System Settings → Spotlight",
        ),
        Err(crate::error::ActionError::Skipped(m)) => {
            StepReport::skipped("spotlight_clipboard", m)
        }
        Err(e) => StepReport::failed("spotlight_clipboard", e.to_string()),
    }
}

fn check_terminal_alias() -> StepReport {
    let Some(home) = directories::UserDirs::new() else {
        return StepReport::skipped("terminal_alias", "no home directory");
    };
    match spotlight::terminal_alias_installed(home.home_dir()) {
        Ok(true) => StepReport::ok(
            "terminal_alias",
            "~/Applications/terminal.app opens Ghostty (Spotlight query: terminal)",
        ),
        Ok(false) => {
            if headless::is_headless() {
                StepReport::skipped(
                    "terminal_alias",
                    "terminal.app alias missing (ok headless; bootstrap installs on GUI Mac)",
                )
            } else {
                StepReport::failed(
                    "terminal_alias",
                    "~/Applications/terminal.app missing; run `sunghyun spotlight restore`",
                )
            }
        }
        Err(crate::error::ActionError::Skipped(m)) => StepReport::skipped("terminal_alias", m),
        Err(e) => StepReport::failed("terminal_alias", e.to_string()),
    }
}

fn check_ax_permission(_headless_mode: bool) -> StepReport {
    // Check-only: never open Settings / poll / Enter (post-switch owns the gate).
    // Probes the binary's *own* grant (responsibility-disclaimed child), so
    // inherited terminal trust cannot false-green this check.
    match ax::accessibility_status() {
        AxGateOutcome::Trusted => StepReport::ok(
            "accessibility",
            "Accessibility granted to the binary itself (disclaimed probe)",
        ),
        AxGateOutcome::Skipped(m) => StepReport::skipped("accessibility", m),
        AxGateOutcome::Failed(m) => StepReport::failed("accessibility", m),
    }
}

fn check_input_monitoring() -> StepReport {
    if headless::is_headless() {
        return StepReport::skipped("input_monitoring", "skipped (headless; Kanata N/A)");
    }
    let kanata_installed = Command::new("sh")
        .args(["-c", "command -v kanata >/dev/null 2>&1"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !kanata_installed {
        return StepReport::skipped("input_monitoring", "kanata not on PATH (opt-in engine)");
    }
    // Only real observable without Full Disk Access: a running kanata holds
    // the IOHID grab, which is impossible without the grant. Anything less is
    // advisory and must not report ok.
    let kanata_running = Command::new("pgrep")
        .args(["-x", "kanata"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if kanata_running {
        StepReport::ok(
            "input_monitoring",
            "kanata running and holding the input grab (grant proven)",
        )
    } else {
        StepReport::skipped(
            "input_monitoring",
            "advisory: kanata installed but not running; Input Monitoring not probeable without FDA (safe-enable proves it)",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_verify_exits_zero() {
        headless::force(true);
        let report = run(&VerifyOpts {
            config_path: None,
            json: false,
            headless: true,
        });
        let plain = report.to_plain();
        assert_eq!(report.exit_code(), 0, "{plain}");
        assert!(report.steps.iter().any(|s| {
            s.id == "spotlight" && s.status == crate::status::StepStatus::Skipped
        }));
        assert!(
            report.steps.iter().any(|s| {
                s.id == "accessibility"
                    && matches!(
                        s.status,
                        crate::status::StepStatus::Skipped | crate::status::StepStatus::Ok
                    )
            }),
            "accessibility missing in headless verify: {plain}"
        );
        headless::clear_force();
    }

    #[test]
    fn verify_does_not_require_configs_tree() {
        headless::force(true);
        let report = run(&VerifyOpts {
            config_path: None,
            json: false,
            headless: true,
        });
        let plain = report.to_plain();
        assert!(
            !plain.contains("Developer/configs"),
            "verify must not depend on configs path: {plain}"
        );
        headless::clear_force();
    }
}
