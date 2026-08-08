use crate::config::Config;
use crate::error::{ActionError, ActionResult};
use crate::headless;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipEntry {
    pub text: String,
    pub ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ClipHistory {
    pub entries: Vec<ClipEntry>,
}

fn history_path() -> PathBuf {
    if let Some(proj) = directories::ProjectDirs::from("com", "anaclumos", "sunghyun") {
        return proj.data_local_dir().join("clipboard-history.json");
    }
    PathBuf::from("/tmp/sunghyun-clipboard-history.json")
}

pub fn load_history() -> ClipHistory {
    let path = history_path();
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => ClipHistory::default(),
    }
}

fn save_history(hist: &ClipHistory) -> ActionResult {
    let path = history_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| ActionError::failed(format!("mkdir clipboard dir: {e}")))?;
    }
    let raw = serde_json::to_string_pretty(hist)
        .map_err(|e| ActionError::failed(format!("serialize clipboard: {e}")))?;
    fs::write(&path, raw).map_err(|e| ActionError::failed(format!("write clipboard: {e}")))?;
    Ok(())
}

fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn capture(config: &Config) -> ActionResult {
    if !config.clipboard.enabled {
        return Err(ActionError::skipped("clipboard disabled in config"));
    }
    let text = read_pasteboard()?;
    if text.trim().is_empty() {
        return Ok(());
    }
    let mut hist = load_history();
    if hist.entries.first().map(|e| e.text.as_str()) == Some(text.as_str()) {
        return Ok(());
    }
    hist.entries.insert(0, ClipEntry { text, ts: now_ts() });
    hist.entries.truncate(config.clipboard.history_limit);
    save_history(&hist)
}

pub fn show(config: &Config) -> ActionResult {
    if !config.clipboard.enabled {
        return Err(ActionError::skipped("clipboard disabled in config"));
    }
    let _ = capture(config);
    let hist = load_history();
    if hist.entries.is_empty() {
        eprintln!("clipboard history empty");
        return Ok(());
    }

    if headless::is_headless() {
        for (i, e) in hist.entries.iter().enumerate() {
            let preview: String = e.text.chars().take(80).collect();
            println!("{i}\t{preview}");
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        macos_pick_and_paste(&hist)
    }
    #[cfg(not(target_os = "macos"))]
    {
        for (i, e) in hist.entries.iter().enumerate() {
            let preview: String = e.text.chars().take(80).collect();
            println!("{i}\t{preview}");
        }
        Ok(())
    }
}

pub fn paste_index(config: &Config, index: usize) -> ActionResult {
    let hist = load_history();
    let Some(entry) = hist.entries.get(index) else {
        return Err(ActionError::failed(format!(
            "clipboard index {index} out of range (len {})",
            hist.entries.len()
        )));
    };
    write_pasteboard(&entry.text)?;
    let _ = config;
    if headless::is_headless() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("osascript")
            .args([
                "-e",
                r#"tell application "System Events" to keystroke "v" using command down"#,
            ])
            .output()
            .map_err(|e| ActionError::failed(format!("osascript paste: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(ActionError::failed(format!(
                "paste keystroke failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

fn read_pasteboard() -> Result<String, ActionError> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("pbpaste")
            .output()
            .map_err(|e| ActionError::failed(format!("pbpaste: {e}")))?;
        if !output.status.success() {
            if headless::is_headless() {
                return Err(ActionError::skipped("pbpaste failed (headless)"));
            }
            return Err(ActionError::failed("pbpaste failed"));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
    #[cfg(target_os = "linux")]
    {
        if headless::is_headless() {
            return Err(ActionError::skipped(
                "clipboard read skipped (no DISPLAY/WAYLAND)",
            ));
        }
        let output = Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output()
            .or_else(|_| Command::new("wl-paste").output())
            .map_err(|e| ActionError::failed(format!("clipboard read: {e}")))?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(ActionError::skipped("clipboard unsupported"))
    }
}

fn write_pasteboard(text: &str) -> ActionResult {
    #[cfg(target_os = "macos")]
    {
        let mut child = Command::new("pbcopy")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| ActionError::failed(format!("pbcopy: {e}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| ActionError::failed(format!("pbcopy write: {e}")))?;
        }
        let status = child
            .wait()
            .map_err(|e| ActionError::failed(format!("pbcopy wait: {e}")))?;
        if status.success() {
            Ok(())
        } else if headless::is_headless() {
            Err(ActionError::skipped("pbcopy failed (headless)"))
        } else {
            Err(ActionError::failed("pbcopy failed"))
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = text;
        if headless::is_headless() {
            Err(ActionError::skipped("clipboard write skipped (headless)"))
        } else {
            Err(ActionError::skipped(
                "clipboard write not implemented on this OS",
            ))
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_pick_and_paste(hist: &ClipHistory) -> ActionResult {
    let labels: Vec<String> = hist
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let preview: String = e.text.chars().take(60).collect();
            let escaped = preview.replace('\\', "\\\\").replace('"', "\\\"");
            format!("{i}: {escaped}")
        })
        .collect();
    let list = labels
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        r#"set opts to {{{list}}}
try
  set choice to choose from list opts with prompt "Clipboard History" OK button name "Paste" cancel button name "Cancel"
  if choice is false then return "CANCEL"
  return item 1 of choice
on error
  return "CANCEL"
end try"#
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| ActionError::failed(format!("osascript: {e}")))?;
    let choice = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if choice.is_empty() || choice == "CANCEL" {
        return Ok(());
    }
    let idx_str = choice.split(':').next().unwrap_or("");
    let idx: usize = idx_str
        .parse()
        .map_err(|_| ActionError::failed(format!("bad choice: {choice}")))?;
    let cfg = Config::default();
    paste_index(&cfg, idx)
}

pub fn push_text_for_test(config: &Config, text: &str) -> ActionResult {
    let mut hist = load_history();
    hist.entries.insert(
        0,
        ClipEntry {
            text: text.into(),
            ts: now_ts(),
        },
    );
    hist.entries.truncate(config.clipboard.history_limit);
    save_history(&hist)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_ring_respects_limit() {
        headless::force(true);
        let mut cfg = Config::default();
        cfg.clipboard.history_limit = 3;
        // isolate path via env would be nicer; exercise truncate logic directly
        let mut hist = ClipHistory::default();
        for i in 0..5 {
            hist.entries.insert(
                0,
                ClipEntry {
                    text: format!("t{i}"),
                    ts: i,
                },
            );
            hist.entries.truncate(cfg.clipboard.history_limit);
        }
        assert_eq!(hist.entries.len(), 3);
        assert_eq!(hist.entries[0].text, "t4");
    }

    #[test]
    fn headless_show_lists_without_gui() {
        headless::force(true);
        let mut cfg = Config::default();
        cfg.clipboard.enabled = true;
        let _ = push_text_for_test(&cfg, "hello-headless");
        assert!(show(&cfg).is_ok());
    }

    #[test]
    fn default_config_disables_clipboard() {
        let cfg = Config::default();
        assert!(!cfg.clipboard.enabled);
        headless::force(true);
        let err = show(&cfg).unwrap_err();
        assert!(matches!(err, crate::error::ActionError::Skipped(_)));
    }
}
