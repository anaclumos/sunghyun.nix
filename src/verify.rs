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
    steps.push(check_virtualization());
    steps.push(check_cursor_agent());
    steps.push(check_coding_cli("codex", "codex", "codex"));
    steps.push(check_coding_cli("claude", "claude", "claude-code"));
    steps.push(check_dia());
    steps.push(check_default_browser());
    steps.push(check_dock());
    steps.push(check_desktop_icons());
    steps.push(check_locale_units());
    steps.push(check_kakaotalk_language());
    steps.push(check_ime_mapping(&config));
    steps.push(check_apps(&config));
    steps.push(check_tiles());
    steps.push(check_clipboard(&config));
    steps.push(check_launcher(&config));
    steps.push(check_keyboard_engine());
    steps.push(check_fn_state());
    steps.push(check_reserved_hotkeys());
    steps.push(check_fn_tap());
    steps.push(check_kanata_config(&config));
    steps.push(check_hushlogin());
    steps.push(check_spotlight());
    steps.push(check_spotlight_clipboard());
    steps.push(check_terminal_alias());
    steps.push(check_menubar());
    steps.push(check_ax_permission(headless_mode));
    steps.push(check_input_monitoring());
    steps.push(check_fonts());

    Report {
        headless: headless_mode,
        steps,
    }
}

fn check_binary_features() -> StepReport {
    StepReport::ok(
        "binary",
        "features: open,default-browser,input-source,tile,launcher,clipboard,verify,post-switch,kanata,virt",
    )
}

/// Informational, never a failure: names the machine class so a run's log
/// shows why the App Store surface behaved the way it did.
fn check_virtualization() -> StepReport {
    StepReport::ok("virtualization", crate::virt::describe())
}

/// OUTCOMES.md row p: Cursor Agent CLI present. macOS installs it through the
/// official `cursor-cli` Homebrew cask declared in the flake; Linux gets the
/// nixpkgs `cursor-cli` package from the portable layer. Either way the binary
/// is `cursor-agent`.
fn check_cursor_agent() -> StepReport {
    let candidates = [
        "/opt/homebrew/bin/cursor-agent",
        "/usr/local/bin/cursor-agent",
    ];
    let home_candidate = directories::UserDirs::new()
        .map(|u| u.home_dir().join(".local/bin/cursor-agent"))
        .filter(|p| p.exists());
    let found = candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|p| (*p).to_string())
        .or_else(|| home_candidate.map(|p| p.display().to_string()))
        .or_else(|| {
            Command::new("sh")
                .args(["-c", "command -v cursor-agent 2>/dev/null"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty())
        });
    match found {
        Some(path) => StepReport::ok("cursor_agent", format!("cursor-agent present ({path})")),
        None if headless::is_headless() => StepReport::skipped(
            "cursor_agent",
            "cursor-agent not on PATH (headless; the portable layer installs it on the next switch)",
        ),
        None if cfg!(target_os = "macos")
            && !std::path::Path::new("/opt/homebrew/bin/brew").exists() =>
        {
            StepReport::skipped(
                "cursor_agent",
                "Homebrew absent, so the cursor-cli cask could not install yet; converges next switch",
            )
        }
        None => StepReport::failed(
            "cursor_agent",
            "cursor-agent missing; the cursor-cli cask (macOS) / nixpkgs cursor-cli (Linux) should have installed it",
        ),
    }
}

/// OUTCOMES.md row v: Codex and Claude Code CLIs present. macOS installs them
/// through the official `codex` and `claude-code` Homebrew casks declared in
/// the flake; Linux gets the same-named nixpkgs packages from the portable
/// layer. The package/cask token and the binary name differ for Claude Code.
fn check_coding_cli(id: &'static str, binary: &str, package: &str) -> StepReport {
    let found = ["/opt/homebrew/bin", "/usr/local/bin"]
        .iter()
        .map(|d| format!("{d}/{binary}"))
        .find(|p| std::path::Path::new(p).exists())
        .or_else(|| {
            Command::new("sh")
                .args(["-c", &format!("command -v {binary} 2>/dev/null")])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty())
        });
    match found {
        Some(path) => StepReport::ok(id, format!("{binary} present ({path})")),
        None if headless::is_headless() => StepReport::skipped(
            id,
            format!("{binary} not on PATH (headless; the portable layer installs it on the next switch)"),
        ),
        None if cfg!(target_os = "macos")
            && !std::path::Path::new("/opt/homebrew/bin/brew").exists() =>
        {
            StepReport::skipped(
                id,
                format!("Homebrew absent, so the {package} cask could not install yet; converges next switch"),
            )
        }
        None => StepReport::failed(
            id,
            format!("{binary} missing; the {package} cask (macOS) / nixpkgs {package} (Linux) should have installed it"),
        ),
    }
}

/// `defaults read`, trimmed. None when the key or the whole domain is absent,
/// which for a sandboxed app also covers "container not readable from here".
fn defaults_read(args: &[&str]) -> Option<String> {
    let output = Command::new("/usr/bin/defaults")
        .arg("read")
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// A nested value out of a user preference domain, as a raw scalar. `defaults
/// read` cannot address a key path and its old-style output loses types, so
/// this exports the typed plist and extracts through it.
fn defaults_extract(domain: &str, key_path: &str) -> Option<String> {
    let export = Command::new("/usr/bin/defaults")
        .args(["export", domain, "-"])
        .output()
        .ok()?;
    if !export.status.success() {
        return None;
    }
    let mut child = Command::new("/usr/bin/plutil")
        .args(["-extract", key_path, "raw", "-o", "-", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    use std::io::Write;
    child.stdin.as_mut()?.write_all(&export.stdout).ok()?;
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// OUTCOMES.md row aa: Dia present. Installed by the `thebrowsercompany-dia`
/// Homebrew cask, which is an official homebrew/cask token.
fn check_dia() -> StepReport {
    if cfg!(not(target_os = "macos")) {
        return StepReport::skipped("dia", "Dia is macOS-only");
    }
    if std::path::Path::new("/Applications/Dia.app").exists() {
        StepReport::ok("dia", "/Applications/Dia.app present")
    } else if headless::is_headless() {
        StepReport::skipped("dia", "Dia absent (headless; the cask installs it on a GUI Mac)")
    } else {
        StepReport::failed(
            "dia",
            "Dia missing; the thebrowsercompany-dia cask should have installed it",
        )
    }
}

/// OUTCOMES.md row ab: Dia is the system default browser, so Hyper+J opens it.
fn check_default_browser() -> StepReport {
    if cfg!(not(target_os = "macos")) {
        return StepReport::skipped("default_browser", "default browser is macOS-only");
    }
    match crate::default_browser::current_handler() {
        Some(id) if id.eq_ignore_ascii_case(crate::default_browser::DIA_BUNDLE_ID) => {
            StepReport::ok("default_browser", format!("http handler is Dia ({id})"))
        }
        Some(id) if headless::is_headless() => StepReport::skipped(
            "default_browser",
            format!("http handler is {id} (headless; the confirmation panel needs a GUI session)"),
        ),
        Some(id) => StepReport::failed(
            "default_browser",
            format!("http handler is {id}; run `sunghyun default-browser set` and answer macOS's panel"),
        ),
        None => StepReport::skipped("default_browser", "LaunchServices reports no http handler"),
    }
}

/// OUTCOMES.md row ac: the Dock holds nothing but its permanent fixtures.
/// Finder and the Trash are not preferences and cannot be removed.
fn check_dock() -> StepReport {
    if cfg!(not(target_os = "macos")) || headless::is_headless() {
        return StepReport::skipped("dock", "Dock state needs a GUI macOS session");
    }
    let pinned = |key: &str| {
        defaults_read(&["com.apple.dock", key])
            .map(|t| t.matches("tile-data").count())
            .unwrap_or(0)
    };
    let apps = pinned("persistent-apps");
    let others = pinned("persistent-others");
    let recents = defaults_read(&["com.apple.dock", "show-recents"])
        .map(|t| t == "0")
        .unwrap_or(false);
    if apps == 0 && others == 0 && recents {
        StepReport::ok("dock", "Dock empty except Finder and the Trash")
    } else {
        StepReport::failed(
            "dock",
            format!("Dock still pinned: {apps} apps, {others} others, show-recents off={recents}"),
        )
    }
}

/// OUTCOMES.md row ad: hard disks on the Desktop, item info under each icon,
/// labels to the right.
fn check_desktop_icons() -> StepReport {
    if cfg!(not(target_os = "macos")) || headless::is_headless() {
        return StepReport::skipped("desktop_icons", "Desktop icons need a GUI macOS session");
    }
    let disks = defaults_read(&["com.apple.finder", "ShowHardDrivesOnDesktop"])
        .map(|t| t == "1")
        .unwrap_or(false);
    let item_info = defaults_extract(
        "com.apple.finder",
        "DesktopViewSettings.IconViewSettings.showItemInfo",
    );
    let label_bottom = defaults_extract(
        "com.apple.finder",
        "DesktopViewSettings.IconViewSettings.labelOnBottom",
    );
    if disks && item_info.as_deref() == Some("true") && label_bottom.as_deref() == Some("false") {
        StepReport::ok(
            "desktop_icons",
            "hard disks shown, item info on, labels on the right",
        )
    } else {
        StepReport::failed(
            "desktop_icons",
            format!(
                "hard disks={disks}, showItemInfo={}, labelOnBottom={}",
                item_info.unwrap_or_else(|| "unset".into()),
                label_bottom.unwrap_or_else(|| "unset".into())
            ),
        )
    }
}

/// OUTCOMES.md row ae: Celsius and metric. macOS reads three separate keys and
/// disagrees with itself when only some are set.
fn check_locale_units() -> StepReport {
    if cfg!(not(target_os = "macos")) || headless::is_headless() {
        return StepReport::skipped("locale_units", "locale units need a GUI macOS session");
    }
    let temp = defaults_read(&["-g", "AppleTemperatureUnit"]);
    let measure = defaults_read(&["-g", "AppleMeasurementUnits"]);
    let metric = defaults_read(&["-g", "AppleMetricUnits"]);
    if temp.as_deref() == Some("Celsius")
        && measure.as_deref() == Some("Centimeters")
        && metric.as_deref() == Some("1")
    {
        StepReport::ok("locale_units", "Celsius, Centimeters, metric")
    } else {
        StepReport::failed(
            "locale_units",
            format!(
                "AppleTemperatureUnit={}, AppleMeasurementUnits={}, AppleMetricUnits={}",
                temp.unwrap_or_else(|| "unset".into()),
                measure.unwrap_or_else(|| "unset".into()),
                metric.unwrap_or_else(|| "unset".into())
            ),
        )
    }
}

/// OUTCOMES.md row af: KakaoTalk runs in Korean whatever the system language
/// is. It is a sandboxed App Store app, so its preference domain redirects
/// into a container that only the app and Language & Region can be sure of
/// reading; an unreadable container is a skip, not a failure.
fn check_kakaotalk_language() -> StepReport {
    if cfg!(not(target_os = "macos")) {
        return StepReport::skipped("kakaotalk_language", "per-app language is macOS-only");
    }
    if headless::is_headless() || !std::path::Path::new("/Applications/KakaoTalk.app").exists() {
        return StepReport::skipped(
            "kakaotalk_language",
            "KakaoTalk not installed here (mas converges it later)",
        );
    }
    match defaults_read(&["com.kakao.KakaoTalkMac", "AppleLanguages"]) {
        Some(text) if text.contains("ko") => {
            StepReport::ok("kakaotalk_language", "KakaoTalk AppleLanguages = ko")
        }
        Some(text) => StepReport::failed(
            "kakaotalk_language",
            format!("KakaoTalk AppleLanguages = {}", text.replace('\n', " ")),
        ),
        None => StepReport::skipped(
            "kakaotalk_language",
            "KakaoTalk's sandbox container is not readable from here; Language & Region owns the value",
        ),
    }
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
                ("hyper+w right three quarters", "tile last-three-fourths"),
                ("hyper browser", "open-default-browser"),
                ("hyper+i iina", "open iina"),
                ("hyper+n slack", "open slack"),
                ("hyper+p preview", "open preview"),
                ("hyper+r linear", "open linear"),
                ("hyper+grave dark mode", "toggle-dark-mode"),
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
    let required = [
        "calendar", "ghostty", "iina", "linear", "mail", "preview", "slack",
    ];
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

/// OUTCOMES.md (o): the media top row is only real if IOHIDSystem agrees with
/// the declared preference, not just the plist.
fn check_fn_state() -> StepReport {
    match crate::fn_state::current_mode() {
        Ok(0) => StepReport::ok("fn_state", "top row fires media bare (HIDFKeyMode=0)"),
        Ok(mode) => StepReport::failed(
            "fn_state",
            format!(
                "IOHIDSystem enforces HIDFKeyMode={mode}; the declared media top row has not converged"
            ),
        ),
        Err(crate::error::ActionError::Skipped(m)) => StepReport::skipped("fn_state", m),
        Err(e) => StepReport::failed("fn_state", e.to_string()),
    }
}

/// OUTCOMES.md row w: ⌘⇧Space belongs to 1Password, so no macOS symbolic hot
/// key may still be sitting on it.
fn check_reserved_hotkeys() -> StepReport {
    match crate::hotkeys::claimants() {
        Ok(found) => {
            let still: Vec<String> = found
                .iter()
                .filter(|c| c.enabled)
                .map(|c| c.describe())
                .collect();
            if still.is_empty() {
                StepReport::ok(
                    "reserved_hotkeys",
                    "⌘⇧Space reaches 1Password only (no system shortcut claims it)",
                )
            } else {
                StepReport::failed("reserved_hotkeys", still.join("; "))
            }
        }
        Err(crate::error::ActionError::Skipped(m)) => StepReport::skipped("reserved_hotkeys", m),
        Err(e) => StepReport::failed("reserved_hotkeys", e.to_string()),
    }
}

/// OUTCOMES.md row u: a bare fn tap opens the Emoji & Symbols picker.
/// AppleFnUsageType governs the bare tap only; the fn+F-row inversion rides
/// HIDFKeyMode (check_fn_state), so the two checks cannot collide.
fn check_fn_tap() -> StepReport {
    if !cfg!(target_os = "macos") {
        return StepReport::skipped("fn_tap", "fn tap behaviour is macOS only");
    }
    if headless::is_headless() {
        return StepReport::skipped("fn_tap", "fn tap check skipped (headless; no keyboard UI)");
    }
    let output = Command::new("/usr/bin/defaults")
        .args(["read", "com.apple.HIToolbox", "AppleFnUsageType"])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let value = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if value == "2" {
                StepReport::ok(
                    "fn_tap",
                    "bare fn tap opens Emoji & Symbols (AppleFnUsageType=2)",
                )
            } else {
                StepReport::failed(
                    "fn_tap",
                    format!(
                        "AppleFnUsageType={value}; expected 2 (Show Emoji & Symbols); darwin-rebuild switch declares it"
                    ),
                )
            }
        }
        // Missing key is the OS default, which already shows Emoji & Symbols.
        Ok(_) => StepReport::ok(
            "fn_tap",
            "AppleFnUsageType unset; macOS defaults the bare fn tap to Emoji & Symbols",
        ),
        Err(e) => StepReport::failed("fn_tap", format!("defaults read AppleFnUsageType: {e}")),
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

/// OUTCOMES.md row v: Sunghyun Sans is visible in the OS font path. macOS
/// materializes nix-darwin `fonts.packages` under /Library/Fonts/Nix Fonts;
/// Linux exposes Home Manager fonts through the profile's share/fonts.
fn check_fonts() -> StepReport {
    fn find_family(dir: &std::path::Path) -> Option<std::path::PathBuf> {
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_family(&path) {
                    return Some(found);
                }
            } else if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("SunghyunSans"))
            {
                return Some(path);
            }
        }
        None
    }
    let mut roots = vec![std::path::PathBuf::from("/Library/Fonts/Nix Fonts")];
    if let Some(user_dirs) = directories::UserDirs::new() {
        roots.push(user_dirs.home_dir().join(".nix-profile/share/fonts"));
    }
    match roots.iter().find_map(|r| find_family(r)) {
        Some(path) => StepReport::ok(
            "fonts",
            format!("Sunghyun Sans installed ({})", path.display()),
        ),
        None if headless::is_headless() => StepReport::skipped(
            "fonts",
            "Sunghyun Sans not found yet (headless; the next switch installs it)",
        ),
        None => StepReport::failed(
            "fonts",
            "Sunghyun Sans missing from /Library/Fonts/Nix Fonts and ~/.nix-profile/share/fonts; darwin-rebuild/home-manager switch installs it",
        ),
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
        assert!(report
            .steps
            .iter()
            .any(|s| { s.id == "spotlight" && s.status == crate::status::StepStatus::Skipped }));
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
