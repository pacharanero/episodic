// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Read-only browser adapter. There is deliberately no generic command endpoint.
//! Episode inspection uses the same engine function as `episode inspect`.

use crate::{channel, episode, lifecycle};
use anyhow::{Context, Result, bail};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path as WebPath, State},
    http::{HeaderMap, Request, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Serialize;
use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Semaphore;

#[derive(Clone)]
struct Server {
    config: PathBuf,
    assets: PathBuf,
    authority: String,
    token: String,
    reads: Arc<Semaphore>,
}

pub fn run(config: Option<PathBuf>, assets: PathBuf, port: u16, dry_run: bool) -> Result<()> {
    let config = channel::resolve_path(config.as_deref())?;
    channel::load(&config)?;
    let assets = assets
        .canonicalize()
        .context("frontend directory not found; run npm ci && npm run build in gui/")?;
    if !assets.join("index.html").is_file() {
        bail!("frontend index.html not found; run npm run build in gui/");
    }
    if dry_run {
        eprintln!("Would serve a read-only workbench on 127.0.0.1:{port}");
        return Ok(());
    }
    tokio::runtime::Runtime::new()?.block_on(async move {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
        let authority = listener.local_addr()?.to_string();
        let token = uuid::Uuid::new_v4().simple().to_string();
        let state = Server {
            config,
            assets,
            authority: authority.clone(),
            token: token.clone(),
            reads: Arc::new(Semaphore::new(2)),
        };
        println!("http://{authority}/#{token}");
        eprintln!("Read-only workbench. Keep the URL private. Press Ctrl+C to stop.");
        axum::serve(listener, router(state)).await?;
        Ok(())
    })
}

fn router(state: Server) -> Router {
    Router::new()
        .route("/api/workspace", get(workspace))
        .route("/", get(index))
        .route("/assets/{*path}", get(asset))
        .layer(middleware::from_fn_with_state(state.clone(), protect))
        .with_state(state)
}

fn authorised(headers: &HeaderMap, state: &Server, api: bool) -> bool {
    if headers.get(header::HOST).and_then(|h| h.to_str().ok()) != Some(&state.authority) {
        return false;
    }
    if let Some(origin) = headers.get(header::ORIGIN)
        && origin.to_str().ok() != Some(&format!("http://{}", state.authority))
    {
        return false;
    }
    !api || headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        == Some(&format!("Bearer {}", state.token))
}

async fn protect(State(state): State<Server>, request: Request<Body>, next: Next) -> Response {
    if !authorised(
        request.headers(),
        &state,
        request.uri().path().starts_with("/api/"),
    ) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    headers.insert(header::CONTENT_SECURITY_POLICY, "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'".parse().unwrap());
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("referrer-policy", "no-referrer".parse().unwrap());
    response
}

async fn index(State(state): State<Server>) -> Response {
    serve_asset(&state.assets, Path::new("index.html")).await
}

async fn asset(State(state): State<Server>, WebPath(path): WebPath<String>) -> Response {
    serve_asset(&state.assets, &Path::new("assets").join(path)).await
}

async fn serve_asset(root: &Path, path: &Path) -> Response {
    if path
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(file) = root.join(path).canonicalize() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !file.starts_with(root) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let content_type = match file.extension().and_then(|s| s.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    match tokio::fs::read(file).await {
        Ok(bytes) => ([(header::CONTENT_TYPE, content_type)], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Serialize)]
struct Workspace {
    channel: channel::ChannelIdentity,
    stages: &'static [&'static str],
    models: Vec<Routing>,
    episodes: Vec<Entry>,
}

#[derive(Serialize)]
struct Routing {
    task: String,
    profile: String,
    provider: String,
    model: String,
}

#[derive(Serialize)]
struct Entry {
    entry: lifecycle::EpisodeEntry,
    report: Option<episode::EpisodeReport>,
    error: Option<String>,
    links: Vec<episode::EpisodeLink>,
}

async fn workspace(State(state): State<Server>) -> Response {
    let Ok(permit) = state.reads.clone().try_acquire_owned() else {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            "Workspace is busy; refresh shortly.",
        )
            .into_response();
    };
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        read_workspace(&state.config)
    })
    .await
    {
        Ok(Ok(data)) => Json(data).into_response(),
        // Do not echo config parse errors or arbitrary file contents into HTTP.
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Could not read workspace. Check the configuration and episode directories.",
        )
            .into_response(),
    }
}

fn read_workspace(config_path: &Path) -> Result<Workspace> {
    let config = channel::load(config_path)?;
    let episodes_path = config.episodes_path(config_path)?;
    let mut entries = Vec::new();
    for entry in lifecycle::list(&episodes_path, None)?.episodes {
        let report = if entry.has_metadata {
            safe_episode(&entry.path, &episodes_path).and_then(|()| episode::inspect(&entry.path))
        } else {
            Err(anyhow::anyhow!("No episode record"))
        };
        let (report, error) = match report {
            Ok(report) => (Some(report), None),
            Err(_) => (
                None,
                Some(
                    if entry.has_metadata {
                        "Episode could not be inspected safely"
                    } else {
                        "No episode record"
                    }
                    .to_owned(),
                ),
            ),
        };
        let links = report
            .as_ref()
            .map(|report| episode::links(&report.fields))
            .unwrap_or_default();
        entries.push(Entry {
            entry,
            report,
            error,
            links,
        });
    }
    let mut models = Vec::new();
    if let Some(policy) = config.llm {
        for (task, profile) in
            std::iter::once(("Default".to_owned(), policy.default_profile)).chain(policy.tasks)
        {
            if let Some(model) = policy.profiles.get(&profile) {
                models.push(Routing {
                    task,
                    profile,
                    provider: model.provider.clone(),
                    model: model.model.clone(),
                });
            }
        }
    }
    Ok(Workspace {
        channel: config.channel,
        stages: lifecycle::STAGES,
        models,
        episodes: entries,
    })
}

// Inspection reads only canonical, top-level artefacts. Refuse symlinks outside
// the configured episodes root before letting that existing reader open them.
fn safe_episode(path: &Path, root: &Path) -> Result<()> {
    let root = root.canonicalize()?;
    if !path.canonicalize()?.starts_with(&root) {
        bail!("episode outside workspace");
    }
    for child in std::fs::read_dir(path)? {
        let child = child?.path();
        if child.is_symlink() && !child.canonicalize()?.starts_with(&root) {
            bail!("artefact outside workspace");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_requires_exact_host_origin_and_api_token() {
        let server = Server {
            config: PathBuf::new(),
            assets: PathBuf::new(),
            authority: "127.0.0.1:1234".into(),
            token: "secret".into(),
            reads: Arc::new(Semaphore::new(2)),
        };
        let mut headers = HeaderMap::new();
        assert!(!authorised(&headers, &server, false));
        headers.insert(header::HOST, server.authority.parse().unwrap());
        assert!(authorised(&headers, &server, false));
        assert!(!authorised(&headers, &server, true));
        headers.insert(header::AUTHORIZATION, "Bearer secret".parse().unwrap());
        assert!(authorised(&headers, &server, true));
        for origin in [
            "https://example.org",
            "null",
            "http://localhost:1234",
            "http://127.0.0.1:9999",
        ] {
            headers.insert(header::ORIGIN, origin.parse().unwrap());
            assert!(!authorised(&headers, &server, true));
        }
        headers.insert(header::ORIGIN, "http://127.0.0.1:1234".parse().unwrap());
        assert!(authorised(&headers, &server, true));
        headers.insert(header::HOST, "attacker.example:1234".parse().unwrap());
        assert!(!authorised(&headers, &server, true));
    }

    #[test]
    fn workspace_exposes_routing_but_not_credential_references() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("eps.toml");
        std::fs::write(&config, "[channel]\nid='fixture'\nname='Fixture'\ntimezone='Europe/London'\n[llm]\ndefault_profile='editor'\n[llm.profiles.editor]\nprovider='anthropic'\nmodel='example'\napi_key_env='DO_NOT_EXPOSE_ME'\n").unwrap();
        let value = serde_json::to_value(read_workspace(&config).unwrap()).unwrap();
        assert_eq!(value["models"][0]["model"], "example");
        assert!(!value.to_string().contains("DO_NOT_EXPOSE_ME"));
        assert_eq!(value["episodes"], serde_json::json!([]));
    }

    #[cfg(unix)]
    #[test]
    fn inspection_refuses_outside_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let folder = root.path().join("2026.11.23-example");
        std::fs::create_dir(&folder).unwrap();
        assert!(safe_episode(&folder, root.path()).is_ok());
        std::fs::write(outside.path().join("secret"), "private").unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret"),
            folder.join("example.tags.txt"),
        )
        .unwrap();
        assert!(safe_episode(&folder, root.path()).is_err());
        assert!(safe_episode(outside.path(), root.path()).is_err());
    }
}
