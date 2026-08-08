use crate::error::{ActionError, ActionResult};
use crate::headless;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Control Center visibility: Don't Show in Menu Bar (live enum on this host).
const CONTROL_CENTER_DONT_SHOW_IN_MENU_BAR: &str = "2";

const CURSOR_APPLICATION_USER_KEY: &str =
    "src.vs.platform.reactivestorage.browser.reactiveStorageServiceImpl.persistentStorage.applicationUser";

pub fn hide_time_machine() -> ActionResult {
    if headless::is_headless() {
        return Err(ActionError::skipped(
            "Time Machine menu bar hide skipped (headless)",
        ));
    }

    #[cfg(target_os = "macos")]
    {
        run_defaults(&[
            "-currentHost",
            "write",
            "com.apple.controlcenter",
            "TimeMachine",
            "-int",
            CONTROL_CENTER_DONT_SHOW_IN_MENU_BAR,
        ])?;
        run_defaults(&[
            "write",
            "com.apple.systemuiserver",
            "NSStatusItem VisibleCC com.apple.menuextra.TimeMachine",
            "-bool",
            "false",
        ])?;
        run_defaults(&[
            "write",
            "com.apple.systemuiserver",
            "NSStatusItem Visible com.apple.menuextra.TimeMachine",
            "-bool",
            "false",
        ])?;
        run_defaults(&["write", "com.apple.systemuiserver", "menuExtras", "-array"])?;
        let _ = Command::new("/usr/bin/killall")
            .arg("SystemUIServer")
            .status();
        let _ = Command::new("/usr/bin/killall")
            .arg("ControlCenter")
            .status();
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(ActionError::skipped(
            "Time Machine menu bar hide is macOS-only",
        ))
    }
}

pub fn is_time_machine_hidden() -> Result<bool, ActionError> {
    #[cfg(target_os = "macos")]
    {
        let visible_cc = defaults_read(&[
            "read",
            "com.apple.systemuiserver",
            "NSStatusItem VisibleCC com.apple.menuextra.TimeMachine",
        ]);
        let visible = defaults_read(&[
            "read",
            "com.apple.systemuiserver",
            "NSStatusItem Visible com.apple.menuextra.TimeMachine",
        ]);
        let cc = defaults_read(&[
            "-currentHost",
            "read",
            "com.apple.controlcenter",
            "TimeMachine",
        ]);

        let not_visible = matches!(visible_cc.as_deref(), Ok("0") | Ok("false"))
            || matches!(visible.as_deref(), Ok("0") | Ok("false"));
        let cc_hidden = matches!(cc.as_deref(), Ok("2"));
        // Prefer explicit systemuiserver hide; CC=2 alone was not enough on this host
        // while menuExtras still listed TimeMachine.menu.
        Ok(not_visible && (cc_hidden || cc.is_err()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(ActionError::skipped(
            "Time Machine menu bar check is macOS-only",
        ))
    }
}

pub fn hide_cursor_tray(home: &Path) -> ActionResult {
    if headless::is_headless() {
        return Err(ActionError::skipped(
            "Cursor menu bar hide skipped (headless)",
        ));
    }

    #[cfg(target_os = "macos")]
    {
        let db = cursor_state_db(home);
        if !db.is_file() {
            return Err(ActionError::skipped(
                "Cursor state.vscdb missing (install/launch Cursor first)",
            ));
        }
        let mut data = read_application_user(&db)?;
        if data.get("systemTrayEnabled") == Some(&Value::Bool(false)) {
            return Ok(());
        }
        data["systemTrayEnabled"] = json!(false);
        write_application_user(&db, &data)?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = home;
        Err(ActionError::skipped("Cursor menu bar hide is macOS-only"))
    }
}

pub fn is_cursor_tray_hidden(home: &Path) -> Result<bool, ActionError> {
    #[cfg(target_os = "macos")]
    {
        let db = cursor_state_db(home);
        if !db.is_file() {
            return Ok(true);
        }
        let data = read_application_user(&db)?;
        Ok(data.get("systemTrayEnabled") == Some(&Value::Bool(false)))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = home;
        Err(ActionError::skipped("Cursor menu bar check is macOS-only"))
    }
}

fn cursor_state_db(home: &Path) -> PathBuf {
    home.join("Library/Application Support/Cursor/User/globalStorage/state.vscdb")
}

fn run_defaults(args: &[&str]) -> ActionResult {
    let status = Command::new("/usr/bin/defaults")
        .args(args)
        .status()
        .map_err(|e| ActionError::failed(format!("defaults {:?}: {e}", args)))?;
    if status.success() {
        Ok(())
    } else {
        Err(ActionError::failed(format!("defaults {:?} failed", args)))
    }
}

fn defaults_read(args: &[&str]) -> Result<String, String> {
    let output = Command::new("/usr/bin/defaults")
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn read_application_user(db: &Path) -> Result<Value, ActionError> {
    let output = Command::new("/usr/bin/sqlite3")
        .args([
            db.to_str().unwrap_or_default(),
            &format!("SELECT value FROM ItemTable WHERE key='{CURSOR_APPLICATION_USER_KEY}';"),
        ])
        .output()
        .map_err(|e| ActionError::failed(format!("sqlite3 read Cursor state: {e}")))?;
    if !output.status.success() {
        return Err(ActionError::failed(format!(
            "sqlite3 read failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return Err(ActionError::failed(
            "Cursor applicationUser storage row missing",
        ));
    }
    serde_json::from_str(&raw)
        .map_err(|e| ActionError::failed(format!("parse Cursor applicationUser JSON: {e}")))
}

fn write_application_user(db: &Path, data: &Value) -> ActionResult {
    let serialized = serde_json::to_string(data)
        .map_err(|e| ActionError::failed(format!("serialize Cursor applicationUser: {e}")))?;
    let escaped = serialized.replace('\'', "''");
    let sql = format!(
        "UPDATE ItemTable SET value='{escaped}' WHERE key='{CURSOR_APPLICATION_USER_KEY}';"
    );
    let status = Command::new("/usr/bin/sqlite3")
        .args([db.to_str().unwrap_or_default(), &sql])
        .status()
        .map_err(|e| ActionError::failed(format!("sqlite3 write Cursor state: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(ActionError::failed(
            "sqlite3 write failed for Cursor systemTrayEnabled",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cursor_hidden_when_flag_false() {
        let v = json!({"systemTrayEnabled": false});
        assert_eq!(v.get("systemTrayEnabled"), Some(&Value::Bool(false)));
    }

    #[test]
    fn control_center_enum_is_two() {
        assert_eq!(CONTROL_CENTER_DONT_SHOW_IN_MENU_BAR, "2");
    }
}
