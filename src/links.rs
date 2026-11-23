// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Links an episode needs, generated from the episode record and channel
//! configuration. Generating a link contacts nothing.
//!
//! Every surface (CLI, MCP, GUI) takes its links from here, so no surface
//! assembles a URL itself. Only `https` URLs are emitted: a configured value
//! with any other scheme, whitespace or control characters is dropped rather
//! than handed to a browser.

use crate::channel::ChannelConfig;
use crate::document::Document;
use crate::episode::{EpisodeFields, EpisodeReport};
use anyhow::Result;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    YoutubeWatch,
    YoutubeStudio,
    YoutubeChannel,
    YoutubeChannelStudio,
    Recording,
    Destination,
    Post,
    Compose,
}

#[derive(Debug, Serialize)]
pub struct Link {
    pub kind: LinkKind,
    pub label: String,
    pub url: String,
}

/// One publicity platform from the episode record, with what a human needs to
/// post by hand: the text, its length against the channel limit, and a
/// composer link where the platform offers one.
#[derive(Debug, Serialize)]
pub struct Publicity {
    pub platform: String,
    pub text: Option<String>,
    /// `draft` when the record holds a reviewed draft, `title_and_link` when
    /// the text is the episode title and watch URL because no draft exists.
    pub text_source: Option<&'static str>,
    pub characters: usize,
    pub max_characters: Option<u32>,
    pub post_url: Option<Link>,
    pub post_date: Option<String>,
    pub compose: Option<Link>,
}

#[derive(Debug, Default, Serialize)]
pub struct EpisodeLinks {
    pub youtube: Vec<Link>,
    pub recording: Vec<Link>,
    pub destinations: Vec<Link>,
    pub publicity: Vec<Publicity>,
}

impl EpisodeLinks {
    pub fn as_text(&self) -> String {
        let mut lines = Vec::new();
        for (heading, links) in [
            ("YouTube", &self.youtube),
            ("Recording", &self.recording),
            ("Channel destinations", &self.destinations),
        ] {
            if links.is_empty() {
                continue;
            }
            lines.push(format!("{heading}:"));
            lines.extend(
                links
                    .iter()
                    .map(|link| format!("- {}: {}", link.label, link.url)),
            );
        }
        if !self.publicity.is_empty() {
            lines.push("Publicity:".to_owned());
            for item in &self.publicity {
                let posted = match &item.post_url {
                    Some(link) => format!("posted {}", link.url),
                    None => "not posted".to_owned(),
                };
                let length = match item.max_characters {
                    Some(limit) => format!("{}/{limit} characters", item.characters),
                    None => format!("{} characters", item.characters),
                };
                lines.push(format!("- {}: {posted}; {length}", item.platform));
                if let Some(compose) = &item.compose {
                    lines.push(format!("  compose: {}", compose.url));
                }
            }
        }
        if lines.is_empty() {
            return "No links: no recorded Video ID, recording links or publicity sections."
                .to_owned();
        }
        lines.join("\n")
    }
}

/// Every link for one episode.
pub fn for_episode(config: &ChannelConfig, report: &EpisodeReport) -> Result<EpisodeLinks> {
    let youtube = youtube(&report.fields);
    let watch = youtube
        .iter()
        .find(|link| link.kind == LinkKind::YoutubeWatch)
        .map(|link| link.url.clone());

    // Recording links are only useful before the episode is out.
    let recording = if matches!(report.stage.as_str(), "in-planning" | "in-progress") {
        recording(config)
    } else {
        Vec::new()
    };

    let destinations = config
        .archive
        .iter()
        .filter_map(|(name, target)| {
            link(
                LinkKind::Destination,
                format!("Archive: {}", humanise(name)),
                target.topic_url.as_deref()?,
            )
        })
        .collect();

    let document = Document::read(&report.metadata_path)?;
    let publicity = document
        .social_sections()
        .into_iter()
        .map(|section| {
            let (text, text_source) = match (section.draft, &watch, &report.fields.title) {
                (Some(draft), _, _) => (Some(draft), Some("draft")),
                (None, Some(url), Some(title)) => {
                    (Some(format!("{title}\n\n{url}")), Some("title_and_link"))
                }
                _ => (None, None),
            };
            let task = format!("{}-draft", section.platform.to_ascii_lowercase());
            let max_characters = config
                .metadata
                .as_ref()
                .and_then(|policy| policy.social.get(&task))
                .and_then(|limits| limits.max_characters);
            let compose = text
                .as_deref()
                .and_then(|text| compose_url(&section.platform, text))
                .map(|url| Link {
                    kind: LinkKind::Compose,
                    label: format!("Compose on {}", section.platform),
                    url,
                });
            Publicity {
                characters: text.as_deref().map_or(0, |text| text.chars().count()),
                post_url: section.post_url.as_deref().and_then(|url| {
                    link(LinkKind::Post, format!("{} post", section.platform), url)
                }),
                platform: section.platform,
                text,
                text_source,
                max_characters,
                post_date: section.post_date,
                compose,
            }
        })
        .collect();

    Ok(EpisodeLinks {
        youtube,
        recording,
        destinations,
        publicity,
    })
}

/// Watch and Studio links from a recorded video ID. Nothing is guessed: an
/// absent or malformed ID produces no links.
pub fn youtube(fields: &EpisodeFields) -> Vec<Link> {
    let Some(id) = fields.video_id.as_deref().filter(|id| is_video_id(id)) else {
        return Vec::new();
    };
    vec![
        Link {
            kind: LinkKind::YoutubeWatch,
            label: "Watch on YouTube".to_owned(),
            url: format!("https://www.youtube.com/watch?v={id}"),
        },
        Link {
            kind: LinkKind::YoutubeStudio,
            label: "Edit in YouTube Studio".to_owned(),
            url: format!("https://studio.youtube.com/video/{id}/edit"),
        },
    ]
}

/// The channel's own YouTube page and Studio dashboard.
pub fn channel(config: &ChannelConfig) -> Vec<Link> {
    let Some(id) = config
        .youtube
        .as_ref()
        .map(|youtube| youtube.channel_id.as_str())
        .filter(|id| is_channel_id(id))
    else {
        return Vec::new();
    };
    vec![
        Link {
            kind: LinkKind::YoutubeChannel,
            label: "Channel on YouTube".to_owned(),
            url: format!("https://www.youtube.com/channel/{id}"),
        },
        Link {
            kind: LinkKind::YoutubeChannelStudio,
            label: "Channel in YouTube Studio".to_owned(),
            url: format!("https://studio.youtube.com/channel/{id}"),
        },
    ]
}

fn recording(config: &ChannelConfig) -> Vec<Link> {
    let mut links = Vec::new();
    for (tool, profile) in &config.recording {
        for (role, url) in [
            ("guest", &profile.guest_url),
            ("host", &profile.host_url),
            ("director", &profile.director_url),
        ] {
            if let Some(found) = url.as_deref().and_then(|url| {
                link(
                    LinkKind::Recording,
                    format!("{} {role}", humanise(tool)),
                    url,
                )
            }) {
                links.push(found);
            }
        }
    }
    links
}

fn link(kind: LinkKind, label: String, url: &str) -> Option<Link> {
    is_safe_url(url).then(|| Link {
        kind,
        label,
        url: url.to_owned(),
    })
}

/// Composer URLs for platforms that accept prefilled text. Platforms without
/// one (Mastodon needs an instance, forums need a topic) get the text to
/// copy instead.
fn compose_url(platform: &str, text: &str) -> Option<String> {
    let encoded = percent_encode(text);
    match platform.to_ascii_lowercase().as_str() {
        "bluesky" => Some(format!("https://bsky.app/intent/compose?text={encoded}")),
        "linkedin" => Some(format!(
            "https://www.linkedin.com/feed/?shareActive=true&text={encoded}"
        )),
        "x" | "twitter" => Some(format!("https://x.com/intent/post?text={encoded}")),
        "threads" => Some(format!(
            "https://www.threads.net/intent/post?text={encoded}"
        )),
        _ => None,
    }
}

/// RFC 3986 percent-encoding of everything except unreserved characters.
fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn is_safe_url(url: &str) -> bool {
    url.len() <= 2048
        && url
            .strip_prefix("https://")
            .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'))
        && !url
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
}

fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_channel_id(id: &str) -> bool {
    id.len() == 24
        && id.starts_with("UC")
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn humanise(key: &str) -> String {
    key.split(['_', '-'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            characters
                .next()
                .map(|first| first.to_uppercase().chain(characters).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_links_use_only_a_valid_recorded_video_id() {
        let mut fields = EpisodeFields::default();
        for id in [
            None,
            Some("[To be added]"),
            Some("javascript:alert(1)"),
            Some("../secrets"),
        ] {
            fields.video_id = id.map(str::to_owned);
            assert!(youtube(&fields).is_empty());
        }
        fields.video_id = Some("dQw4w9WgXcQ".into());
        let links = youtube(&fields);
        assert_eq!(links[0].url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!(
            links[1].url,
            "https://studio.youtube.com/video/dQw4w9WgXcQ/edit"
        );
    }

    #[test]
    fn only_https_urls_reach_a_browser() {
        for url in [
            "javascript:alert(1)",
            "http://example.org",
            "https://",
            "https:///path",
            "https://example.org/a b",
            "https://example.org/\u{0007}",
        ] {
            assert!(!is_safe_url(url), "{url}");
        }
        assert!(is_safe_url("https://vdo.ninja/?room=example&push=guest"));
    }

    #[test]
    fn compose_text_is_fully_encoded() {
        assert_eq!(
            compose_url("Bluesky", "A & B #tag\nhttps://x.y/?a=1").unwrap(),
            "https://bsky.app/intent/compose?text=A%20%26%20B%20%23tag%0Ahttps%3A%2F%2Fx.y%2F%3Fa%3D1"
        );
        assert_eq!(compose_url("Mastodon", "text"), None);
        assert_eq!(percent_encode("é"), "%C3%A9");
    }

    #[test]
    fn configuration_keys_read_as_labels() {
        assert_eq!(humanise("vdo_ninja"), "Vdo Ninja");
        assert_eq!(humanise("forum"), "Forum");
    }
}
