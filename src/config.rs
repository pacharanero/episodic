use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

/// Loads and provides access to configuration from environment variables and env files.
pub struct Config {
    values: HashMap<String, String>,
}

impl Config {
    /// Load configuration from an env file (if it exists) and merge with environment variables.
    pub fn load_from_env_file(path: &str) -> Result<Self> {
        let mut values = HashMap::new();
        if Path::new(path).is_file() {
            let content = fs::read_to_string(path).context("Failed to read edh-live.env")?;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let mut parts = trimmed.splitn(2, '=');
                if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                    values.insert(key.trim().to_string(), value.trim().to_string());
                }
            }
        }
        Ok(Self { values })
    }

    /// Fetch a required configuration value. Checks environment variables first, then the env file.
    pub fn fetch_required(&mut self, key: &str) -> Result<String> {
        if let Ok(env_val) = std::env::var(key) {
            if !env_val.trim().is_empty() {
                return Ok(env_val);
            }
        }
        if let Some(val) = self.values.get(key) {
            if !val.trim().is_empty() {
                return Ok(val.clone());
            }
        }
        Err(anyhow!(
            "Missing required environment variable: {} (set it in edh-live.env)",
            key
        ))
    }

    /// Fetch an optional configuration value with a fallback default.
    pub fn fetch_optional(&mut self, key: &str, fallback: &str) -> String {
        if let Ok(env_val) = std::env::var(key) {
            if !env_val.trim().is_empty() {
                return env_val;
            }
        }
        self.values
            .get(key)
            .cloned()
            .filter(|val| !val.trim().is_empty())
            .unwrap_or_else(|| fallback.to_string())
    }
}
