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

/// Extract a Google resource ID from a URL or return the value as-is if it's already an ID.
///
/// Supports:
/// - Google Drive folders: `https://drive.google.com/drive/folders/ID` or `https://drive.google.com/drive/u/0/folders/ID`
/// - Google Docs: `https://docs.google.com/document/d/ID/edit`
/// - Plain IDs (returned unchanged)
pub fn extract_google_id(url_or_id: &str) -> String {
    let trimmed = url_or_id.trim();

    // Google Drive folder URL patterns
    // https://drive.google.com/drive/folders/ID
    // https://drive.google.com/drive/u/0/folders/ID
    if trimmed.contains("drive.google.com") && trimmed.contains("/folders/") {
        if let Some(id_start) = trimmed.rfind("/folders/") {
            let after_folders = &trimmed[id_start + 9..];
            // Take everything up to the next / or ? or end
            let id = after_folders
                .split(&['/', '?'][..])
                .next()
                .unwrap_or(after_folders);
            if !id.is_empty() {
                return id.to_string();
            }
        }
    }

    // Google Docs URL pattern
    // https://docs.google.com/document/d/ID/edit
    if trimmed.contains("docs.google.com/document/d/") {
        if let Some(id_start) = trimmed.find("/document/d/") {
            let after_d = &trimmed[id_start + 12..];
            // Take everything up to the next /
            let id = after_d.split('/').next().unwrap_or(after_d);
            if !id.is_empty() {
                return id.to_string();
            }
        }
    }

    // Not a recognized URL, return as-is (assumed to be an ID already)
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_google_id_drive_folder() {
        assert_eq!(
            extract_google_id(
                "https://drive.google.com/drive/folders/1PT6uejyWSBCRxm7WkYfw7ZoviL0CdL_k"
            ),
            "1PT6uejyWSBCRxm7WkYfw7ZoviL0CdL_k"
        );
    }

    #[test]
    fn test_extract_google_id_drive_folder_with_user() {
        assert_eq!(
            extract_google_id(
                "https://drive.google.com/drive/u/0/folders/1PT6uejyWSBCRxm7WkYfw7ZoviL0CdL_k"
            ),
            "1PT6uejyWSBCRxm7WkYfw7ZoviL0CdL_k"
        );
    }

    #[test]
    fn test_extract_google_id_docs() {
        assert_eq!(
            extract_google_id("https://docs.google.com/document/d/1PS-WIo7WhZt41Ss7kIP6Wd8HOoAWv-KTIzI3Yhp2GpE/edit"),
            "1PS-WIo7WhZt41Ss7kIP6Wd8HOoAWv-KTIzI3Yhp2GpE"
        );
    }

    #[test]
    fn test_extract_google_id_plain_id() {
        assert_eq!(
            extract_google_id("1PT6uejyWSBCRxm7WkYfw7ZoviL0CdL_k"),
            "1PT6uejyWSBCRxm7WkYfw7ZoviL0CdL_k"
        );
    }

    #[test]
    fn test_extract_google_id_with_query_params() {
        assert_eq!(
            extract_google_id("https://drive.google.com/drive/folders/1PT6uejy?usp=sharing"),
            "1PT6uejy"
        );
    }
}
