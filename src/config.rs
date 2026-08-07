use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config not found: {0}")]
    NotFound(PathBuf),
    #[error("failed to read config: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid config: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default)]
    pub ime: ImeConfig,
    #[serde(default)]
    pub apps: BTreeMap<String, AppEntry>,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    #[serde(default)]
    pub tiles: TileConfig,
    #[serde(default)]
    pub launcher: LauncherConfig,
    #[serde(default)]
    pub clipboard: ClipboardConfig,
    #[serde(default)]
    pub paths: PathsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImeConfig {
    #[serde(default = "default_abc")]
    pub abc: String,
    #[serde(default = "default_korean")]
    pub korean_2set: String,
}

fn default_abc() -> String {
    "com.apple.keylayout.ABC".into()
}

fn default_korean() -> String {
    "com.apple.inputmethod.Korean.2SetKorean".into()
}

impl Default for ImeConfig {
    fn default() -> Self {
        Self {
            abc: default_abc(),
            korean_2set: default_korean(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppEntry {
    pub bundle_id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TileConfig {
    #[serde(default = "default_gap")]
    pub gap: i64,
}

fn default_gap() -> i64 {
    0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LauncherConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipboardConfig {
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,
    /// Off by default: use Spotlight Clipboard Search (⌘Space then ⌘4).
    #[serde(default = "default_false")]
    pub enabled: bool,
}

fn default_history_limit() -> usize {
    50
}

fn default_false() -> bool {
    false
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            history_limit: default_history_limit(),
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PathsConfig {
    pub kanata_kbd: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ime: ImeConfig::default(),
            apps: default_apps(),
            aliases: default_aliases(),
            tiles: TileConfig { gap: 0 },
            launcher: LauncherConfig::default(),
            clipboard: ClipboardConfig::default(),
            paths: PathsConfig::default(),
        }
    }
}

fn default_apps() -> BTreeMap<String, AppEntry> {
    let mut m = BTreeMap::new();
    for (key, bundle_id, name) in [
        ("calendar", "com.apple.iCal", "Calendar"),
        ("kakaotalk", "com.kakao.KakaoTalkMac", "KakaoTalk"),
        ("linear", "com.linear", "Linear"),
        ("mail", "com.apple.mail", "Mail"),
        ("music", "com.apple.Music", "Music"),
        ("slack", "com.tinyspeck.slackmacgap", "Slack"),
        ("ghostty", "com.mitchellh.ghostty", "Ghostty"),
        ("tableplus", "com.tinyapp.TablePlus", "TablePlus"),
    ] {
        m.insert(
            key.into(),
            AppEntry {
                bundle_id: Some(bundle_id.into()),
                name: Some(name.into()),
            },
        );
    }
    m
}

fn default_aliases() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("terminal".into(), "ghostty".into());
    m.insert("planetscale".into(), "tableplus".into());
    m
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.to_path_buf()));
        }
        let raw = fs::read_to_string(path)?;
        Self::parse(&raw)
    }

    pub fn parse(raw: &str) -> Result<Self, ConfigError> {
        toml::from_str(raw).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    pub fn resolve_ime_id(&self, name: &str) -> Option<String> {
        match name.to_ascii_lowercase().as_str() {
            "abc" | "english" | "en" => Some(self.ime.abc.clone()),
            "korean" | "2set" | "2setkorean" | "ko" => Some(self.ime.korean_2set.clone()),
            _ => {
                if name.contains('.') {
                    Some(name.to_string())
                } else {
                    None
                }
            }
        }
    }

    pub fn resolve_app(&self, key: &str) -> Option<&AppEntry> {
        if let Some(entry) = self.apps.get(key) {
            return Some(entry);
        }
        if let Some(alias_target) = self.aliases.get(key) {
            return self.apps.get(alias_target);
        }
        None
    }
}

pub fn default_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(p) = env_path("SUNGHYUN_CONFIG") {
        paths.push(p);
    }
    if let Some(proj) = directories::ProjectDirs::from("com", "anaclumos", "sunghyun") {
        paths.push(proj.config_dir().join("sunghyun.toml"));
    }
    if let Some(home) = directories::UserDirs::new() {
        paths.push(home.home_dir().join(".config/sunghyun/sunghyun.toml"));
    }
    paths.push(PathBuf::from("sunghyun.toml"));
    paths
}

fn env_path(name: &str) -> Result<PathBuf, std::env::VarError> {
    Ok(PathBuf::from(std::env::var(name)?))
}

pub fn load_or_default(explicit: Option<&Path>) -> Result<(Config, Option<PathBuf>), ConfigError> {
    if let Some(p) = explicit {
        return Ok((Config::load(p)?, Some(p.to_path_buf())));
    }
    for p in default_config_paths() {
        if p.exists() {
            return Ok((Config::load(&p)?, Some(p)));
        }
    }
    Ok((Config::default(), None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_and_ime_map() {
        let cfg = Config::parse(
            r#"
[ime]
abc = "com.apple.keylayout.ABC"
korean_2set = "com.apple.inputmethod.Korean.2SetKorean"

[apps.mail]
bundle_id = "com.apple.mail"
name = "Mail"

[aliases]
terminal = "ghostty"
"#,
        )
        .unwrap();
        assert_eq!(
            cfg.resolve_ime_id("ABC").as_deref(),
            Some("com.apple.keylayout.ABC")
        );
        assert_eq!(
            cfg.resolve_ime_id("2SetKorean").as_deref(),
            Some("com.apple.inputmethod.Korean.2SetKorean")
        );
        assert_eq!(
            cfg.resolve_app("mail").unwrap().bundle_id.as_deref(),
            Some("com.apple.mail")
        );
        assert!(cfg.resolve_app("terminal").is_none()); // ghostty not in this minimal file
    }

    #[test]
    fn default_apps_and_aliases() {
        let cfg = Config::default();
        assert_eq!(
            cfg.resolve_app("terminal").unwrap().name.as_deref(),
            Some("Ghostty")
        );
        assert_eq!(
            cfg.resolve_app("planetscale").unwrap().name.as_deref(),
            Some("TablePlus")
        );
        assert_eq!(cfg.tiles.gap, 0);
        assert_eq!(cfg.clipboard.history_limit, 50);
        assert!(!cfg.clipboard.enabled);
    }

    #[test]
    fn reject_bad_toml() {
        let err = Config::parse("[[[not valid").unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }
}
