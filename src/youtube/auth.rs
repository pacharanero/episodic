// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reusing the workspace's existing OAuth credentials.
//!
//! The consent flow has already been completed and the stored token carries a
//! refresh token, so `eps` never needs to open a browser. It reads the client
//! secret and the token that the previous tooling produced, and hands them to
//! the authorised-user flow, which refreshes without user interaction.

use anyhow::{Context, Result, bail};
use google_youtube3::yup_oauth2::authorized_user::AuthorizedUserSecret;
use serde::Deserialize;
use std::path::Path;

/// A Google client secret file, in either of the shapes Google issues.
#[derive(Debug, Deserialize)]
struct ClientSecretFile {
    #[serde(default)]
    installed: Option<ClientSecret>,
    #[serde(default)]
    web: Option<ClientSecret>,
}

#[derive(Debug, Deserialize)]
struct ClientSecret {
    client_id: String,
    client_secret: String,
}

/// The stored token. Only the refresh token matters: access tokens expire in
/// an hour and are replaced automatically.
#[derive(Debug, Deserialize)]
struct StoredToken {
    #[serde(default)]
    refresh_token: Option<String>,
}

/// Build credentials from the workspace's existing files.
pub fn authorized_user(
    client_secret_path: &Path,
    token_path: &Path,
) -> Result<AuthorizedUserSecret> {
    let secret_source = std::fs::read_to_string(client_secret_path).with_context(|| {
        format!(
            "could not read the Google client secret at {}",
            client_secret_path.display()
        )
    })?;
    let file: ClientSecretFile = serde_json::from_str(&secret_source).with_context(|| {
        format!(
            "{} is not a Google client secret file",
            client_secret_path.display()
        )
    })?;
    let secret = file
        .installed
        .or(file.web)
        .context("client secret file has neither an 'installed' nor a 'web' section")?;

    let token_source = std::fs::read_to_string(token_path).with_context(|| {
        format!(
            "could not read the stored OAuth token at {}; authorise once with the existing tooling first",
            token_path.display()
        )
    })?;
    let token: StoredToken = serde_json::from_str(&token_source)
        .with_context(|| format!("{} is not a stored OAuth token", token_path.display()))?;

    let Some(refresh_token) = token.refresh_token.filter(|value| !value.is_empty()) else {
        bail!(
            "{} has no refresh token, so it cannot be renewed without re-authorising",
            token_path.display()
        );
    };

    Ok(AuthorizedUserSecret {
        client_id: secret.client_id,
        client_secret: secret.client_secret,
        refresh_token,
        key_type: "authorized_user".to_owned(),
    })
}

/// Where a channel keeps its credentials, resolved against the workspace.
pub fn credential_paths(
    config: &crate::channel::ChannelConfig,
    workspace_root: &Path,
) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let youtube = config
        .credentials
        .get("youtube")
        .context("this channel has no [credentials.youtube] section")?;
    let resolve = |path: &Path| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            workspace_root.join(path)
        }
    };
    let client_secret = credential_file_path(
        youtube.client_secret_path.as_deref(),
        youtube.client_secret_env.as_deref(),
        "credentials.youtube.client_secret",
        &resolve,
    )?;
    let token = credential_file_path(
        youtube.token_path.as_deref(),
        youtube.token_env.as_deref(),
        "credentials.youtube.token",
        &resolve,
    )?;
    Ok((client_secret, token))
}

fn credential_file_path(
    path: Option<&Path>,
    environment: Option<&str>,
    field: &str,
    resolve: &impl Fn(&Path) -> std::path::PathBuf,
) -> Result<std::path::PathBuf> {
    match (path, environment) {
        (Some(path), None) => Ok(resolve(path)),
        (None, Some(environment)) => {
            let value = std::env::var_os(environment)
                .with_context(|| format!("{field}_env is not set: {environment}"))?;
            if value.is_empty() {
                bail!("{field}_env is set but empty: {environment}");
            }
            Ok(resolve(Path::new(&value)))
        }
        (None, None) => bail!("{field}_path or {field}_env is not set"),
        (Some(_), Some(_)) => bail!("{field}_path and {field}_env cannot both be set"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(directory: &Path, name: &str, contents: &str) -> std::path::PathBuf {
        let path = directory.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn reads_either_client_secret_shape() {
        let directory = tempfile::tempdir().unwrap();
        let token = write(
            directory.path(),
            "token.json",
            r#"{"access_token":"a","refresh_token":"r"}"#,
        );
        for (name, source) in [
            (
                "web.json",
                r#"{"web":{"client_id":"id","client_secret":"s"}}"#,
            ),
            (
                "installed.json",
                r#"{"installed":{"client_id":"id","client_secret":"s"}}"#,
            ),
        ] {
            let secret = write(directory.path(), name, source);
            let credentials = authorized_user(&secret, &token).unwrap();
            assert_eq!(credentials.client_id, "id");
            assert_eq!(credentials.refresh_token, "r");
        }
    }

    #[test]
    fn resolves_credential_file_paths_from_environment_references() {
        let directory = tempfile::tempdir().unwrap();
        let variable = format!("EPISODIC_TEST_TOKEN_PATH_{}", std::process::id());
        unsafe {
            std::env::set_var(&variable, "credentials/token.json");
        }
        let config: crate::channel::ChannelConfig = toml::from_str(&format!(
            "[channel]\nid = 'test'\nname = 'Test'\ntimezone = 'UTC'\n[credentials.youtube]\nclient_secret_path = 'credentials/client.json'\ntoken_env = '{variable}'\n"
        ))
        .unwrap();
        let paths = credential_paths(&config, directory.path()).unwrap();
        assert_eq!(paths.0, directory.path().join("credentials/client.json"));
        assert_eq!(paths.1, directory.path().join("credentials/token.json"));
        unsafe {
            std::env::remove_var(variable);
        }
    }

    #[test]
    fn refuses_a_token_that_cannot_be_refreshed() {
        let directory = tempfile::tempdir().unwrap();
        let secret = write(
            directory.path(),
            "secret.json",
            r#"{"web":{"client_id":"id","client_secret":"s"}}"#,
        );
        let token = write(directory.path(), "token.json", r#"{"access_token":"a"}"#);
        let error = authorized_user(&secret, &token).unwrap_err().to_string();
        assert!(error.contains("no refresh token"), "{error}");
    }

    #[test]
    fn explains_a_missing_token_rather_than_failing_obscurely() {
        let directory = tempfile::tempdir().unwrap();
        let secret = write(
            directory.path(),
            "secret.json",
            r#"{"web":{"client_id":"id","client_secret":"s"}}"#,
        );
        let error = authorized_user(&secret, &directory.path().join("absent.json"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("authorise once"), "{error}");
    }
}
