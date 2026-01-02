use anyhow::{Context, Result};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379/";

#[derive(Debug, Clone)]
pub struct Config {
    pub redis_url: String,
}

#[derive(Debug, Deserialize)]
struct FileConfig {
    redis_url: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let file_config: FileConfig = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config file: {}", path.display()))?;

        Ok(Self {
            redis_url: file_config
                .redis_url
                .unwrap_or_else(|| DEFAULT_REDIS_URL.to_string()),
        })
    }

}

impl Default for Config {
    fn default() -> Self {
        Self {
            redis_url: DEFAULT_REDIS_URL.to_string(),
        }
    }
}

fn config_path() -> PathBuf {
    if let Ok(dir) = env::var("XDG_CONFIG_HOME") {
        return Path::new(&dir).join("zellij-driver").join("config.toml");
    }

    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Path::new(&home)
        .join(".config")
        .join("zellij-driver")
        .join("config.toml")
}
