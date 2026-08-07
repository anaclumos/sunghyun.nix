use crate::error::{ActionError, ActionResult};
use crate::headless;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Apple symbolic hotkey id 64 = Show Spotlight search (⌘Space).
pub const SPOTLIGHT_HOTKEY_ID: &str = "64";

/// Bundle id for the thin Spotlight name alias that opens Ghostty.
pub const TERMINAL_ALIAS_BUNDLE_ID: &str = "com.anaclumos.terminal-ghostty";

/// Ghostty bundle id (open target for the terminal alias app).
pub const GHOSTTY_BUNDLE_ID: &str = "com.mitchellh.ghostty";

/// Restore Spotlight ⌘Space (enabled). Raycast no longer owns ⌘Space.
pub fn restore_command_space() -> ActionResult {
    if headless::is_headless() {
        return Err(ActionError::skipped(
            "Spotlight restore skipped (headless / no GUI session)",
        ));
    }

    #[cfg(target_os = "macos")]
    {
        set_symbolic_hotkey_enabled(true)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(ActionError::skipped("Spotlight restore is macOS-only"))
    }
}

/// Enable Spotlight Clipboard Search preference (PasteboardHistoryEnabled).
///
/// Apple documents Clipboard Search as ⌘Space then ⌘4 — not a global ⌘⇧V.
/// See https://support.apple.com/guide/mac-help/search-your-clipboard-history-mchl40d5b86b/mac
pub fn enable_pasteboard_history() -> ActionResult {
    if headless::is_headless() {
        return Err(ActionError::skipped(
            "Spotlight pasteboard history skipped (headless)",
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("/usr/bin/defaults")
            .args([
                "write",
                "com.apple.Spotlight",
                "PasteboardHistoryEnabled",
                "-bool",
                "true",
            ])
            .status()
            .map_err(|e| ActionError::failed(format!("defaults write PasteboardHistory: {e}")))?;
        if !status.success() {
            return Err(ActionError::failed(
                "defaults write failed for PasteboardHistoryEnabled",
            ));
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(ActionError::skipped(
            "Spotlight pasteboard history is macOS-only",
        ))
    }
}

/// Spotlight Clipboard Search cannot be opened by this CLI on macOS 26+.
///
/// Tahoe has **no** global ⌘⇧V symbolic hotkey for Clipboard Search
/// (`AppleSymbolicHotKeys` has no clipboard entry; Apple Support documents
/// ⌘Space then ⌘4). Every synthesized-keystroke route was tried live on
/// 2026-08-08 and is dead: osascript System Events keystrokes fail TCC
/// (error 1002) under spawning chains, and CGEventPost events — flags-only
/// and HIDSystemState-source with explicit modifier press alike — are
/// delivered but dropped by WindowServer before the global hotkey matcher
/// (`CGXSenderCanSynthesizeEvents` gates synthetic senders since Sequoia).
/// Only IOHIDSystem-level events trigger symbolic hotkeys, which is exactly
/// what the karabiner.json ⌘⇧V rule emits through the Karabiner DriverKit
/// virtual keyboard. That rule is the supported path; this command reports
/// the truth instead of pretending to work.
/// See https://support.apple.com/guide/mac-help/search-your-clipboard-history-mchl40d5b86b/mac
pub fn open_clipboard_search() -> ActionResult {
    if headless::is_headless() {
        return Err(ActionError::skipped(
            "Spotlight clipboard open skipped (headless)",
        ));
    }
    Err(ActionError::failed(
        "cannot open Clipboard Search from a CLI: macOS 26+ WindowServer drops synthesized keystrokes before the global hotkey matcher; press ⌘⇧V (karabiner virtual-HID rule) or ⌘Space then ⌘4 on the keyboard",
    ))
}

/// Read whether Spotlight Clipboard Search appears enabled.
pub fn is_pasteboard_history_enabled() -> Result<bool, ActionError> {
    #[cfg(target_os = "macos")]
    {
        if headless::is_headless() {
            return Err(ActionError::skipped(
                "Spotlight pasteboard check skipped (headless)",
            ));
        }
        let output = Command::new("/usr/bin/defaults")
            .args([
                "read",
                "com.apple.Spotlight",
                "PasteboardHistoryEnabled",
            ])
            .output()
            .map_err(|e| ActionError::failed(format!("defaults read PasteboardHistory: {e}")))?;
        if !output.status.success() {
            // Missing key → treat as OS default (enabled on Tahoe when Clipboard Search is on).
            return Ok(true);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let t = text.trim();
        Ok(t == "1" || t.eq_ignore_ascii_case("true"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(ActionError::skipped(
            "Spotlight pasteboard check is macOS-only",
        ))
    }
}

/// Path to `~/Applications/terminal.app` (Spotlight query "terminal" → Ghostty).
pub fn terminal_alias_app_path(home: &Path) -> PathBuf {
    home.join("Applications/terminal.app")
}

/// Install a reversible thin app named `terminal` that opens Ghostty.
///
/// Spotlight Quick Keys are for actions, not app aliases. Naming this app
/// `terminal` makes typing "terminal" in Spotlight match Ghostty without
/// deleting Apple Terminal.app.
pub fn install_terminal_ghostty_alias(home: &Path) -> ActionResult {
    if headless::is_headless() {
        return Err(ActionError::skipped(
            "terminal→Ghostty alias skipped (headless)",
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let app = terminal_alias_app_path(home);
        if terminal_alias_is_current(&app)? {
            return Ok(());
        }
        write_terminal_alias_app(&app)?;
        let _ = Command::new("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister")
            .args(["-f", app.to_str().unwrap_or("")])
            .status();
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = home;
        Err(ActionError::skipped(
            "terminal→Ghostty alias is macOS-only",
        ))
    }
}

/// Whether `~/Applications/terminal.app` is present with the expected bundle id.
pub fn terminal_alias_installed(home: &Path) -> Result<bool, ActionError> {
    #[cfg(target_os = "macos")]
    {
        terminal_alias_is_current(&terminal_alias_app_path(home))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = home;
        Err(ActionError::skipped(
            "terminal→Ghostty alias check is macOS-only",
        ))
    }
}

/// Read whether Spotlight ⌘Space appears enabled.
pub fn is_command_space_enabled() -> Result<bool, ActionError> {
    #[cfg(target_os = "macos")]
    {
        if headless::is_headless() {
            return Err(ActionError::skipped("Spotlight check skipped (headless)"));
        }
        read_symbolic_hotkey_enabled()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(ActionError::skipped("Spotlight check is macOS-only"))
    }
}

#[cfg(target_os = "macos")]
fn set_symbolic_hotkey_enabled(enabled: bool) -> ActionResult {
    let enabled_xml = if enabled { "<true/>" } else { "<false/>" };
    let xml = format!(
        "<dict><key>enabled</key>{enabled_xml}<key>value</key><dict><key>type</key><string>standard</string><key>parameters</key><array><integer>32</integer><integer>49</integer><integer>1048576</integer></array></dict></dict>"
    );
    let status = Command::new("/usr/bin/defaults")
        .args([
            "write",
            "com.apple.symbolichotkeys",
            "AppleSymbolicHotKeys",
            "-dict-add",
            SPOTLIGHT_HOTKEY_ID,
            &xml,
        ])
        .status()
        .map_err(|e| ActionError::failed(format!("defaults write Spotlight: {e}")))?;
    if !status.success() {
        return Err(ActionError::failed(
            "defaults write failed for Spotlight hotkey 64",
        ));
    }
    let _ = Command::new("/System/Library/PrivateFrameworks/SystemAdministration.framework/Resources/activateSettings")
        .arg("-u")
        .status();
    Ok(())
}

#[cfg(target_os = "macos")]
fn read_symbolic_hotkey_enabled() -> Result<bool, ActionError> {
    let output = Command::new("/usr/bin/defaults")
        .args([
            "read",
            "com.apple.symbolichotkeys",
            &format!("AppleSymbolicHotKeys.{SPOTLIGHT_HOTKEY_ID}.enabled"),
        ])
        .output()
        .map_err(|e| ActionError::failed(format!("defaults read Spotlight: {e}")))?;
    if !output.status.success() {
        return Ok(true);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let t = text.trim();
    Ok(t == "1" || t.eq_ignore_ascii_case("true"))
}

#[cfg(target_os = "macos")]
fn terminal_alias_is_current(app: &Path) -> Result<bool, ActionError> {
    let plist = app.join("Contents/Info.plist");
    let exe = app.join("Contents/MacOS/terminal");
    if !plist.is_file() || !exe.is_file() {
        return Ok(false);
    }
    let raw = fs::read_to_string(&plist)
        .map_err(|e| ActionError::failed(format!("read {}: {e}", plist.display())))?;
    let exe_raw = fs::read_to_string(&exe)
        .map_err(|e| ActionError::failed(format!("read {}: {e}", exe.display())))?;
    Ok(raw.contains(TERMINAL_ALIAS_BUNDLE_ID)
        && raw.contains("<string>terminal</string>")
        && exe_raw.contains(GHOSTTY_BUNDLE_ID))
}

#[cfg(target_os = "macos")]
fn write_terminal_alias_app(app: &Path) -> ActionResult {
    let contents = app.join("Contents");
    let macos = contents.join("MacOS");
    fs::create_dir_all(&macos)
        .map_err(|e| ActionError::failed(format!("mkdir {}: {e}", macos.display())))?;

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>terminal</string>
	<key>CFBundleExecutable</key>
	<string>terminal</string>
	<key>CFBundleIdentifier</key>
	<string>{TERMINAL_ALIAS_BUNDLE_ID}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>terminal</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>1.0</string>
	<key>CFBundleVersion</key>
	<string>1</string>
	<key>LSMinimumSystemVersion</key>
	<string>14.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSAppleScriptEnabled</key>
	<false/>
</dict>
</plist>
"#
    );
    let exe = format!(
        "#!/bin/bash\nexec /usr/bin/open -b {GHOSTTY_BUNDLE_ID} \"$@\"\n"
    );
    let plist_path = contents.join("Info.plist");
    let exe_path = macos.join("terminal");
    let pkginfo = contents.join("PkgInfo");
    fs::write(&plist_path, plist)
        .map_err(|e| ActionError::failed(format!("write {}: {e}", plist_path.display())))?;
    fs::write(&exe_path, exe)
        .map_err(|e| ActionError::failed(format!("write {}: {e}", exe_path.display())))?;
    fs::write(&pkginfo, "APPL????")
        .map_err(|e| ActionError::failed(format!("write {}: {e}", pkginfo.display())))?;
    let mut perms = fs::metadata(&exe_path)
        .map_err(|e| ActionError::failed(format!("stat {}: {e}", exe_path.display())))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&exe_path, perms)
        .map_err(|e| ActionError::failed(format!("chmod {}: {e}", exe_path.display())))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_restore_skips() {
        headless::force(true);
        let err = restore_command_space().unwrap_err();
        assert!(matches!(err, ActionError::Skipped(_)));
        headless::clear_force();
    }

    #[test]
    fn headless_pasteboard_skips() {
        headless::force(true);
        let err = enable_pasteboard_history().unwrap_err();
        assert!(matches!(err, ActionError::Skipped(_)));
        headless::clear_force();
    }

    #[test]
    fn headless_terminal_alias_skips() {
        headless::force(true);
        let err = install_terminal_ghostty_alias(Path::new("/tmp")).unwrap_err();
        assert!(matches!(err, ActionError::Skipped(_)));
        headless::clear_force();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn write_terminal_alias_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        fs::create_dir_all(home.join("Applications")).unwrap();
        headless::clear_force();
        // Force non-headless path for unit write (install still checks headless).
        // Call writer directly:
        let app = terminal_alias_app_path(home);
        write_terminal_alias_app(&app).unwrap();
        assert!(terminal_alias_is_current(&app).unwrap());
        let exe = fs::read_to_string(app.join("Contents/MacOS/terminal")).unwrap();
        assert!(exe.contains(GHOSTTY_BUNDLE_ID));
    }
}
