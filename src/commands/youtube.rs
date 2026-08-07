// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! YouTube commands.
//!
//! Every remote write is behind `--apply`. Without it the command reports the
//! plan and stops, so the default of any command in this family is to change
//! nothing.

use crate::channel;
use crate::output::{self, OutputFormat};
use crate::youtube::{Destination, api::YouTubeApi, auth, publish};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn connect(config_path: &Path) -> Result<YouTubeApi> {
    let config = channel::load(config_path)?;
    let workspace = channel::workspace_root(config_path)?;
    let (client_secret, token) = auth::credential_paths(&config, &workspace)?;
    YouTubeApi::connect(&client_secret, &token)
}

/// An RFC 3339 timestamp, for recording when something happened.
fn now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default();
    // Days from the civil epoch, by Howard Hinnant's algorithm.
    let days = (seconds / 86_400) as i64;
    let time = seconds % 86_400;
    let era_days = days + 719_468;
    let era = era_days.div_euclid(146_097);
    let day_of_era = era_days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time / 3600,
        (time % 3600) / 60,
        time % 60
    )
}

/// Show what an upload would do, without contacting the destination.
pub fn plan_upload(
    config_path: &Path,
    episode: &Path,
    playlists: &[String],
    privacy: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let config = channel::load(config_path)?;
    let plan = publish::plan(episode, &config, playlists, privacy)?;
    output::write(format, &plan, &plan.as_text())
}

/// Upload an episode and record what the destination confirms.
pub fn upload(
    config_path: &Path,
    episode: &Path,
    playlists: &[String],
    privacy: Option<&str>,
    apply: bool,
    format: OutputFormat,
) -> Result<()> {
    let config = channel::load(config_path)?;
    let plan = publish::plan(episode, &config, playlists, privacy)?;

    if !apply {
        let text = format!(
            "{}\n\nThis is a plan. Pass --apply to upload.",
            plan.as_text()
        );
        return output::write(format, &plan, &text);
    }
    if !plan.blockers.is_empty() {
        bail!(
            "this episode is not ready to upload: {}",
            plan.blockers.join("; ")
        );
    }

    eprintln!("Uploading {} ...", plan.request.file.display());
    let destination = connect(config_path)?;
    let outcome = publish::apply(&destination, &plan, &now())?;
    let complete = outcome.is_complete();
    output::write(format, &outcome, &outcome.as_text())?;
    if !complete {
        bail!(
            "some steps could not be confirmed; the video ID is recorded, so they can be retried"
        );
    }
    Ok(())
}

/// Compare the episode record against the destination.
pub fn verify(config_path: &Path, episode: &Path, format: OutputFormat) -> Result<()> {
    let config = channel::load(config_path)?;
    let destination = connect(config_path)?;
    let report = publish::verify(&destination, episode, &config)?;
    let clean = report.issues.is_empty();
    output::write(format, &report, &report.as_text())?;
    if !clean {
        bail!("verification found differences between the record and the destination");
    }
    Ok(())
}

#[derive(Serialize)]
struct PrivacyChange {
    video_id: String,
    from: Option<String>,
    to: String,
    applied: bool,
}

/// Make a video public. Deliberately separate from upload.
pub fn publish_video(
    config_path: &Path,
    episode: &Path,
    privacy: &str,
    apply: bool,
    format: OutputFormat,
) -> Result<()> {
    crate::youtube::api::validate_privacy(privacy)?;
    let report = crate::episode::inspect(episode)?;
    let video_id = report
        .fields
        .video_id
        .clone()
        .context("this episode has no recorded video ID")?;

    if !apply {
        let change = PrivacyChange {
            video_id: video_id.clone(),
            from: report.fields.privacy_status.clone(),
            to: privacy.to_owned(),
            applied: false,
        };
        let text = format!(
            "Would set {video_id} to {privacy}\n\nThis is a plan. Pass --apply to change it."
        );
        return output::write(format, &change, &text);
    }

    let destination = connect(config_path)?;
    destination.set_privacy(&video_id, privacy)?;

    // Read the value back rather than assuming the call did what was asked.
    let confirmed = destination
        .video(&video_id)?
        .and_then(|state| state.privacy_status)
        .unwrap_or_default();
    if confirmed != privacy {
        bail!("asked for {privacy} but the destination reports '{confirmed}'");
    }

    let (_, metadata_path, _) = crate::episode::locate(episode)?;
    let mut document = crate::document::Document::read(&metadata_path)?;
    if document
        .set_field("Privacy Status", None, &confirmed, true)
        .is_ok_and(|change| change.is_some())
    {
        document.write()?;
    }

    let change = PrivacyChange {
        video_id,
        from: report.fields.privacy_status,
        to: confirmed,
        applied: true,
    };
    let text = format!("Set {} to {}", change.video_id, change.to);
    output::write(format, &change, &text)
}

#[cfg(test)]
mod tests {
    use super::now;

    #[test]
    fn formats_a_timestamp_the_record_can_carry() {
        let stamp = now();
        assert_eq!(stamp.len(), 20, "{stamp}");
        assert!(stamp.ends_with('Z'));
        assert_eq!(&stamp[4..5], "-");
        assert_eq!(&stamp[10..11], "T");
        let year: u32 = stamp[..4].parse().unwrap();
        assert!((2026..2100).contains(&year), "{stamp}");
    }
}
