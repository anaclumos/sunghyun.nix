//! Residual gate machinery for `sunghyun post-switch`.
//!
//! The legacy imperative `bootstrap` subcommand is gone: nix-darwin +
//! Home Manager own packages, files, defaults, and launchd. This module keeps
//! only the manifest (skip lists, DriverKit URL, masApps mirror) and the sudo
//! keep-alive shared by post-switch and `kanata enable --safe`.

pub mod steps;
pub mod sudo_keepalive;

use crate::assets;
use serde::{Deserialize, Serialize};

pub use steps::{StepContext, StepOutcome};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct BootstrapManifest {
    /// Mirror of nix-darwin `homebrew.masApps` (canonical: nix/darwin/modules/homebrew.nix).
    #[serde(default)]
    pub mas_apps: Vec<MasApp>,
    #[serde(default)]
    pub kanata_driver_url: Option<String>,
    #[serde(default)]
    pub skip: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MasApp {
    pub id: u64,
    pub name: String,
}

pub fn load_embedded_manifest() -> BootstrapManifest {
    toml::from_str(assets::MANIFEST_TOML).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_parse_includes_xcode() {
        let m = load_embedded_manifest();
        assert!(
            m.mas_apps.iter().any(|a| a.id == 497799835 && a.name == "Xcode"),
            "{m:?}"
        );
        assert!(m.mas_apps.iter().any(|a| a.name == "KakaoTalk"));
        assert!(m.mas_apps.iter().any(|a| a.name == "What Watt?"));
    }

    #[test]
    fn no_hardcoded_configs_developer_path_in_embedded_kbd() {
        assert!(!assets::KANATA_KBD.contains("Developer/configs"));
        assert!(assets::KANATA_KBD.contains("@lcmd"));
        assert!(assets::KANATA_KBD.contains("@rcmd"));
        assert!(!assets::KANATA_KBD.contains("clipboard show"));
        assert!(assets::KANATA_KBD.contains("tile maximize"));
        assert!(assets::KANATA_KBD.contains("M-spc") && assets::KANATA_KBD.contains("M-4"));
        assert!(assets::KANATA_KBD.contains("open-default-browser"));
        assert!(assets::KANATA_KBD.contains(
            "HOME_DIR_PLACEHOLDER/.config/sunghyun/run-sunghyun.sh"
        ));
    }
}
