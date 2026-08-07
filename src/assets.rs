use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub const BREWFILE: &str = include_str!("../assets/Brewfile");
pub const MANIFEST_TOML: &str = include_str!("../assets/manifest.toml");
pub const KANATA_KBD: &str = include_str!("../assets/kanata.kbd");
pub const SUNGHYUN_TOML: &str = include_str!("../assets/sunghyun.toml");
pub const RUN_WRAPPER: &str = include_str!("../assets/run-sunghyun.sh");

pub const HOME_PLACEHOLDER: &str = "HOME_DIR_PLACEHOLDER";

pub fn config_dir(home: &Path) -> PathBuf {
    home.join(".config/sunghyun")
}

pub fn substitute_home(template: &str, home: &Path) -> String {
    template.replace(HOME_PLACEHOLDER, &home.to_string_lossy())
}

pub fn write_file(path: &Path, contents: &str, executable: bool) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    // Nix-first: on flake-managed machines these files are read-only Home
    // Manager symlinks into /nix/store. Content match = done (never rewrite).
    if fs::read_to_string(path).ok().as_deref() == Some(contents) {
        return Ok(());
    }
    fs::write(path, contents).map_err(|e| format!("write {}: {e}", path.display()))?;
    if executable {
        let mut perms = fs::metadata(path)
            .map_err(|e| format!("stat {}: {e}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)
            .map_err(|e| format!("chmod {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Materialize shipped keyboard + Brewfile + wrapper under ~/.config/sunghyun.
pub fn materialize_runtime_config(home: &Path) -> Result<PathBuf, String> {
    let dir = config_dir(home);
    write_file(
        &dir.join("kanata.kbd"),
        &substitute_home(KANATA_KBD, home),
        false,
    )?;
    write_file(&dir.join("sunghyun.toml"), SUNGHYUN_TOML, false)?;
    write_file(&dir.join("run-sunghyun.sh"), RUN_WRAPPER, true)?;
    write_file(&dir.join("Brewfile"), BREWFILE, false)?;
    write_file(&dir.join("manifest.toml"), MANIFEST_TOML, false)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn substitute_replaces_placeholder() {
        let out = substitute_home("cmd HOME_DIR_PLACEHOLDER/x.sh", Path::new("/Users/sc"));
        assert_eq!(out, "cmd /Users/sc/x.sh");
    }

    #[test]
    fn materialize_writes_met_ime_maximize_and_spotlight_clip() {
        let dir = tempdir().unwrap();
        let home = dir.path();
        materialize_runtime_config(home).unwrap();
        let kbd = fs::read_to_string(home.join(".config/sunghyun/kanata.kbd")).unwrap();
        assert!(kbd.contains("tap-hold-press"));
        assert!(kbd.contains("@lcmd"));
        assert!(kbd.contains("@rcmd"));
        assert!(kbd.contains("lmet"));
        assert!(kbd.contains("rmet"));
        assert!(!kbd.contains("clipboard show"));
        assert!(kbd.contains("tile maximize"));
        assert!(kbd.contains("M-spc") && kbd.contains("M-4"));
        assert!(kbd.contains("open-default-browser"));
        assert!(kbd.contains("@obrowser"));
        assert!(!kbd.contains("open arc"));
        assert!(!kbd.contains("f17"));
        assert!(!kbd.contains("f18"));
        assert!(!kbd.contains("Developer/configs"));
        assert!(kbd.contains(&format!(
            "{}/.config/sunghyun/run-sunghyun.sh",
            home.display()
        )));
        assert!(home.join(".config/sunghyun/Brewfile").is_file());
        let toml = fs::read_to_string(home.join(".config/sunghyun/sunghyun.toml")).unwrap();
        assert!(toml.contains("[clipboard]"));
        assert!(toml.contains("enabled = false"));
    }

    #[test]
    fn embedded_manifest_has_xcode() {
        assert!(MANIFEST_TOML.contains("497799835"));
        assert!(MANIFEST_TOML.contains("Xcode"));
        assert!(MANIFEST_TOML.contains("KakaoTalk"));
        assert!(MANIFEST_TOML.contains("What Watt?"));
    }
}
