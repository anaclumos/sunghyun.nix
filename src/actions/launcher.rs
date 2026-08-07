use crate::config::Config;
use crate::error::{ActionError, ActionResult};
use crate::headless;
use std::fs;
use std::path::PathBuf;

/// Optional alternate app chooser. Default Mac setup restores Apple Spotlight on ⌘Space;
/// this launcher is not bound to ⌘Space unless the owner adds an explicit hotkey.
pub fn launch(config: &Config, query: Option<&str>) -> ActionResult {
    if !config.launcher.enabled {
        return Err(ActionError::skipped("launcher disabled in config"));
    }

    let apps = list_apps();
    let q = query.unwrap_or("").trim().to_ascii_lowercase();
    let mut matches: Vec<(String, PathBuf)> = apps
        .into_iter()
        .filter(|(name, _)| q.is_empty() || name.to_ascii_lowercase().contains(&q))
        .collect();
    matches.sort_by(|a, b| a.0.cmp(&b.0));

    if matches.is_empty() {
        return Err(ActionError::failed(format!("no apps matched query {q:?}")));
    }

    if headless::is_headless() {
        for (name, path) in matches.iter().take(30) {
            println!("{name}\t{}", path.display());
        }
        return Ok(());
    }

    if let Some(q) = query {
        if let Some((name, _)) = matches
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(q) || n.to_ascii_lowercase().contains(q))
        {
            return crate::actions::open::open_by_name(name);
        }
    }

    #[cfg(target_os = "macos")]
    {
        macos_choose(&matches)
    }
    #[cfg(not(target_os = "macos"))]
    {
        for (name, path) in matches.iter().take(30) {
            println!("{name}\t{}", path.display());
        }
        Ok(())
    }
}

fn list_apps() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut dirs = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
    ];
    if let Some(u) = directories::UserDirs::new() {
        dirs.push(u.home_dir().join("Applications"));
    }
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("app") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("app")
                    .to_string();
                out.push((name, path));
            }
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn macos_choose(matches: &[(String, PathBuf)]) -> ActionResult {
    use std::process::Command;
    let labels: Vec<String> = matches
        .iter()
        .take(40)
        .map(|(n, _)| n.replace('\\', "\\\\").replace('"', "\\\""))
        .collect();
    let list = labels
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        r#"set opts to {{{list}}}
try
  set choice to choose from list opts with prompt "sunghyun launcher (optional; Spotlight owns ⌘Space)" OK button name "Open" cancel button name "Cancel"
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
    crate::actions::open::open_by_name(&choice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_lists_without_panic() {
        headless::force(true);
        let cfg = Config::default();
        let _ = launch(&cfg, Some("finder"));
    }
}
