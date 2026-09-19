use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::actions::Action;
use crate::i18n::Lang;

pub const CONFIG_NAME: &str = "config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub buttons: ButtonConfig,
    /// Run software-remap daemon (needed for non-hardware actions).
    #[serde(default = "default_true")]
    pub software_remap: bool,
    /// Last time the user saved to mouse firmware.
    #[serde(default)]
    pub saved_to_mouse: bool,
    /// UI language (English default).
    #[serde(default)]
    pub language: Lang,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonConfig {
    /// Rear side button (firmware index 3, default Back/5).
    pub side: Action,
    /// Front side button (firmware index 4, default Forward/4).
    pub extra: Action,
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            buttons: ButtonConfig {
                side: Action::Default,
                extra: Action::Default,
            },
            software_remap: true,
            saved_to_mouse: false,
            language: Lang::En,
        }
    }
}

pub fn default_config_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("r8ctl");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config/r8ctl");
    }
    PathBuf::from("/tmp/r8ctl")
}

pub fn load_config(path: &Path) -> Config {
    match fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save_config(path: &Path, cfg: &Config) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let s = serde_json::to_string_pretty(cfg).unwrap_or_else(|_| "{}".into());
    fs::write(path, s + "\n")
}
