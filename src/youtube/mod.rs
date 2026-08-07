// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The YouTube destination.
//!
//! Command logic depends on the [`Destination`] trait rather than on the API
//! client, so plan, apply, verify, and record can all be tested without a
//! network or credentials. [`api`] holds the only code that talks to Google.

pub mod api;
pub mod auth;
pub mod publish;

use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// What a video looks like remotely.
#[derive(Clone, Debug, Serialize)]
pub struct VideoState {
    pub id: String,
    pub title: Option<String>,
    pub privacy_status: Option<String>,
    pub published_at: Option<String>,
}

/// Everything needed to create a video.
#[derive(Clone, Debug, Serialize)]
pub struct UploadRequest {
    pub file: PathBuf,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub category_id: String,
    pub privacy_status: String,
    pub language: Option<String>,
    pub license: Option<String>,
}

/// The seven operations `eps` needs from YouTube.
///
/// Reads are separated from writes so a caller can see at a glance which
/// methods can change the destination.
pub trait Destination {
    fn video(&self, id: &str) -> Result<Option<VideoState>>;
    fn caption_count(&self, video_id: &str) -> Result<usize>;
    fn playlist_contains(&self, playlist_id: &str, video_id: &str) -> Result<bool>;

    fn upload_video(&self, request: &UploadRequest) -> Result<String>;
    fn set_privacy(&self, video_id: &str, privacy: &str) -> Result<()>;
    fn set_thumbnail(&self, video_id: &str, image: &Path) -> Result<()>;
    fn upload_caption(
        &self,
        video_id: &str,
        track: &Path,
        language: &str,
        name: &str,
    ) -> Result<()>;
    fn add_to_playlist(&self, playlist_id: &str, video_id: &str) -> Result<()>;
}

#[cfg(test)]
pub mod fake {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeSet;

    /// An in-memory destination for testing command behaviour.
    #[derive(Default)]
    pub struct FakeDestination {
        pub videos: RefCell<Vec<VideoState>>,
        pub playlists: RefCell<BTreeSet<(String, String)>>,
        pub captions: RefCell<Vec<String>>,
        pub thumbnails: RefCell<Vec<String>>,
        /// Operations that should fail, by name.
        pub failing: BTreeSet<String>,
        pub next_id: RefCell<u32>,
    }

    impl FakeDestination {
        fn guard(&self, operation: &str) -> Result<()> {
            if self.failing.contains(operation) {
                anyhow::bail!("{operation} refused by the fake destination");
            }
            Ok(())
        }
    }

    impl Destination for FakeDestination {
        fn video(&self, id: &str) -> Result<Option<VideoState>> {
            self.guard("video")?;
            Ok(self
                .videos
                .borrow()
                .iter()
                .find(|video| video.id == id)
                .cloned())
        }

        fn caption_count(&self, video_id: &str) -> Result<usize> {
            self.guard("caption_count")?;
            Ok(self
                .captions
                .borrow()
                .iter()
                .filter(|id| *id == video_id)
                .count())
        }

        fn playlist_contains(&self, playlist_id: &str, video_id: &str) -> Result<bool> {
            self.guard("playlist_contains")?;
            Ok(self
                .playlists
                .borrow()
                .contains(&(playlist_id.to_owned(), video_id.to_owned())))
        }

        fn upload_video(&self, request: &UploadRequest) -> Result<String> {
            self.guard("upload_video")?;
            let mut next = self.next_id.borrow_mut();
            *next += 1;
            let id = format!("fake-video-{next}");
            self.videos.borrow_mut().push(VideoState {
                id: id.clone(),
                title: Some(request.title.clone()),
                privacy_status: Some(request.privacy_status.clone()),
                published_at: None,
            });
            Ok(id)
        }

        fn set_privacy(&self, video_id: &str, privacy: &str) -> Result<()> {
            self.guard("set_privacy")?;
            for video in self.videos.borrow_mut().iter_mut() {
                if video.id == video_id {
                    video.privacy_status = Some(privacy.to_owned());
                }
            }
            Ok(())
        }

        fn set_thumbnail(&self, video_id: &str, _image: &Path) -> Result<()> {
            self.guard("set_thumbnail")?;
            self.thumbnails.borrow_mut().push(video_id.to_owned());
            Ok(())
        }

        fn upload_caption(
            &self,
            video_id: &str,
            _track: &Path,
            _language: &str,
            _name: &str,
        ) -> Result<()> {
            self.guard("upload_caption")?;
            self.captions.borrow_mut().push(video_id.to_owned());
            Ok(())
        }

        fn add_to_playlist(&self, playlist_id: &str, video_id: &str) -> Result<()> {
            self.guard("add_to_playlist")?;
            self.playlists
                .borrow_mut()
                .insert((playlist_id.to_owned(), video_id.to_owned()));
            Ok(())
        }
    }
}
