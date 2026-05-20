use std::path::PathBuf;
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default)]
    pub close_to_tray: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            close_to_tray: false,
        }
    }
}

fn config_path() -> Result<PathBuf> {
    let home_dir = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE").context("USERPROFILE not set")?
    } else {
        std::env::var("HOME").context("HOME not set")?
    };
    Ok(PathBuf::from(home_dir).join(".backyard").join("config.json"))
}

pub fn load() -> AppConfig {
    let path = match config_path() {
        Ok(p) => p,
        Err(_) => return AppConfig::default(),
    };
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        AppConfig::default()
    }
}

pub fn save(cfg: &AppConfig) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&path, json)?;
    Ok(())
}
