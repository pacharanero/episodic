// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The only code that talks to Google.
//!
//! The async runtime is contained here. `eps` is a one-shot process with
//! nothing to overlap, so the rest of the engine stays synchronous and each
//! call blocks on its own future rather than colouring the whole command tree.

use super::{Destination, UploadRequest, VideoState};
use anyhow::{Context, Result, bail};
use google_youtube3::api::{
    Playlist, PlaylistItem, PlaylistItemSnippet, ResourceId, Video, VideoSnippet, VideoStatus,
};
use google_youtube3::common::Body;
use google_youtube3::{YouTube, hyper_rustls, hyper_util, yup_oauth2};
use std::path::Path;
use tokio::runtime::Runtime;

type Client = google_youtube3::YouTube<
    hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>,
>;

pub struct YouTubeApi {
    runtime: Runtime,
    hub: Client,
}

impl YouTubeApi {
    /// Connect using the workspace's existing credentials.
    pub fn connect(client_secret: &Path, token: &Path) -> Result<Self> {
        install_crypto_provider();
        let credentials = super::auth::authorized_user(client_secret, token)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("could not start the async runtime")?;

        let hub = runtime.block_on(async move {
            let connector = hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()
                .context("no system certificate roots are available")?
                .https_only()
                .enable_http1()
                .enable_http2()
                .build();
            // The hub and the authenticator want different body types, so
            // each gets its own client over the same connector.
            let client: hyper_util::client::legacy::Client<_, Body> =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                    .build(connector.clone());
            let auth_client =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
                    .build(connector);

            let auth = yup_oauth2::AuthorizedUserAuthenticator::with_client(
                credentials,
                yup_oauth2::client::CustomHyperClientBuilder::from(auth_client),
            )
            .build()
            .await
            .context("could not authenticate with the stored refresh token")?;

            Ok::<_, anyhow::Error>(YouTube::new(client, auth))
        })?;

        Ok(Self { runtime, hub })
    }
}

/// Choose a TLS backend explicitly.
///
/// Both `ring` and `aws-lc-rs` end up in the dependency graph through
/// different crates, and rustls refuses to guess between them: without this it
/// panics on the first connection. Installing once is idempotent, and a
/// competing install by another component is not an error.
fn install_crypto_provider() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

impl Destination for YouTubeApi {
    fn video(&self, id: &str) -> Result<Option<VideoState>> {
        let response = self.runtime.block_on(
            self.hub
                .videos()
                .list(&vec!["id".into(), "snippet".into(), "status".into()])
                .add_id(id)
                .doit(),
        );
        let (_, list) = response.context("could not read the video from YouTube")?;
        Ok(list
            .items
            .unwrap_or_default()
            .into_iter()
            .next()
            .map(|video| {
                let snippet = video.snippet.unwrap_or_default();
                VideoState {
                    id: video.id.unwrap_or_else(|| id.to_owned()),
                    title: snippet.title,
                    privacy_status: video.status.and_then(|status| status.privacy_status),
                    published_at: snippet.published_at.map(|at| at.to_rfc3339()),
                }
            }))
    }

    fn caption_count(&self, video_id: &str) -> Result<usize> {
        let (_, list) = self
            .runtime
            .block_on(
                self.hub
                    .captions()
                    .list(&vec!["id".into()], video_id)
                    .doit(),
            )
            .context("could not list captions")?;
        Ok(list.items.unwrap_or_default().len())
    }

    fn playlist_contains(&self, playlist_id: &str, video_id: &str) -> Result<bool> {
        let (_, list) = self
            .runtime
            .block_on(
                self.hub
                    .playlist_items()
                    .list(&vec!["snippet".into()])
                    .playlist_id(playlist_id)
                    .video_id(video_id)
                    .max_results(1)
                    .doit(),
            )
            .context("could not read the playlist")?;
        Ok(!list.items.unwrap_or_default().is_empty())
    }

    /// Upload a video, resuming rather than restarting if the transfer breaks.
    ///
    /// Episode masters are gigabytes over a domestic connection, where a
    /// single-shot upload that fails near the end costs the whole transfer.
    fn upload_video(&self, request: &UploadRequest) -> Result<String> {
        let file = std::fs::File::open(&request.file)
            .with_context(|| format!("could not open {}", request.file.display()))?;
        let video = Video {
            snippet: Some(VideoSnippet {
                title: Some(request.title.clone()),
                description: Some(request.description.clone()),
                tags: Some(request.tags.clone()),
                category_id: Some(request.category_id.clone()),
                default_language: request.language.clone(),
                default_audio_language: request.language.clone(),
                ..VideoSnippet::default()
            }),
            status: Some(VideoStatus {
                privacy_status: Some(request.privacy_status.clone()),
                license: request.license.clone(),
                self_declared_made_for_kids: Some(false),
                ..VideoStatus::default()
            }),
            ..Video::default()
        };

        let (_, uploaded) = self
            .runtime
            .block_on(
                self.hub
                    .videos()
                    .insert(video)
                    .upload_resumable(file, "video/*".parse().expect("a valid MIME type")),
            )
            .context("the upload failed")?;

        uploaded
            .id
            .context("YouTube accepted the upload but returned no video ID")
    }

    fn set_privacy(&self, video_id: &str, privacy: &str) -> Result<()> {
        let video = Video {
            id: Some(video_id.to_owned()),
            status: Some(VideoStatus {
                privacy_status: Some(privacy.to_owned()),
                ..VideoStatus::default()
            }),
            ..Video::default()
        };
        self.runtime
            .block_on(self.hub.videos().update(video).add_part("status").doit())
            .with_context(|| format!("could not set {video_id} to {privacy}"))?;
        Ok(())
    }

    fn set_thumbnail(&self, video_id: &str, image: &Path) -> Result<()> {
        let file = std::fs::File::open(image)
            .with_context(|| format!("could not open {}", image.display()))?;
        let mime = match image.extension().and_then(|e| e.to_str()) {
            Some("jpg" | "jpeg") => "image/jpeg",
            _ => "image/png",
        };
        self.runtime
            .block_on(
                self.hub
                    .thumbnails()
                    .set(video_id)
                    .upload(file, mime.parse().expect("a valid MIME type")),
            )
            .context("the thumbnail was rejected")?;
        Ok(())
    }

    fn upload_caption(
        &self,
        video_id: &str,
        track: &Path,
        language: &str,
        name: &str,
    ) -> Result<()> {
        use google_youtube3::api::{Caption, CaptionSnippet};
        let file = std::fs::File::open(track)
            .with_context(|| format!("could not open {}", track.display()))?;
        let caption = Caption {
            snippet: Some(CaptionSnippet {
                video_id: Some(video_id.to_owned()),
                language: Some(language.to_owned()),
                name: Some(name.to_owned()),
                ..CaptionSnippet::default()
            }),
            ..Caption::default()
        };
        self.runtime
            .block_on(
                self.hub.captions().insert(caption).upload(
                    file,
                    "application/octet-stream"
                        .parse()
                        .expect("a valid MIME type"),
                ),
            )
            .context("the caption track was rejected")?;
        Ok(())
    }

    fn add_to_playlist(&self, playlist_id: &str, video_id: &str) -> Result<()> {
        let item = PlaylistItem {
            snippet: Some(PlaylistItemSnippet {
                playlist_id: Some(playlist_id.to_owned()),
                resource_id: Some(ResourceId {
                    kind: Some("youtube#video".to_owned()),
                    video_id: Some(video_id.to_owned()),
                    ..ResourceId::default()
                }),
                ..PlaylistItemSnippet::default()
            }),
            ..PlaylistItem::default()
        };
        self.runtime
            .block_on(self.hub.playlist_items().insert(item).doit())
            .with_context(|| format!("could not add {video_id} to playlist {playlist_id}"))?;
        Ok(())
    }
}

/// Keep the unused import honest: `Playlist` documents that playlists
/// themselves are read through the same hub if that is ever needed.
#[allow(dead_code)]
fn _playlist_type_is_available(_: Option<Playlist>) {}

/// Reject a privacy value the API would not understand before spending a call.
pub fn validate_privacy(value: &str) -> Result<()> {
    if !matches!(value, "private" | "unlisted" | "public") {
        bail!("privacy must be private, unlisted, or public, got '{value}'");
    }
    Ok(())
}
