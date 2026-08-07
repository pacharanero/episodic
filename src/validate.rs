// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Task contracts for generated metadata.
//!
//! These decide whether a model's answer is publishable. Everything a channel
//! could reasonably disagree about, such as how many tags it wants or what
//! must appear in a description, is configuration; only structural rules are
//! fixed here.

use crate::channel::MetadataPolicy;
use crate::transcript;
use anyhow::{Result, bail};

/// Emoji blocks. The channel voice forbids them and models reach for them.
fn contains_emoji(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(character as u32,
            0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0xFE0F | 0x2190..=0x21FF)
    })
}

fn words(text: &str) -> usize {
    text.split(|character: char| {
        !character.is_alphanumeric() && character != '\'' && character != '-'
    })
    .filter(|word| !word.is_empty())
    .count()
}

fn non_empty_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

/// Validate one task's output.
pub fn check(
    task: &str,
    text: &str,
    policy: &MetadataPolicy,
    transcript_end: Option<u64>,
) -> Result<()> {
    match task {
        "chapters" => chapters(text, policy, transcript_end),
        "tags" => tags(text, policy),
        "thumbnail-quote" => thumbnail_quote(text, policy),
        "youtube-description" => description(text, policy),
        "bluesky-draft" | "mastodon-draft" | "linkedin-draft" | "instagram-caption" => {
            social(task, text, policy)
        }
        "guest-research" => guest_research(text),
        other => bail!("unknown task: {other}"),
    }
}

fn chapters(text: &str, policy: &MetadataPolicy, transcript_end: Option<u64>) -> Result<()> {
    let lines = non_empty_lines(text);
    if lines.is_empty() {
        bail!("output is empty");
    }
    if lines.len() < policy.chapters_min as usize || lines.len() > policy.chapters_max as usize {
        bail!(
            "chapters must contain {}-{} lines, got {}",
            policy.chapters_min,
            policy.chapters_max,
            lines.len()
        );
    }
    if lines[0] != policy.first_chapter {
        bail!(
            "first chapter must be exactly \"{}\", got {:?}",
            policy.first_chapter,
            lines[0]
        );
    }

    let mut previous: i64 = -1;
    for line in &lines {
        let (timestamp, title) = line
            .split_once(char::is_whitespace)
            .filter(|(_, title)| !title.trim().is_empty())
            .with_context_line(line)?;
        let seconds = transcript::parse_timestamp(timestamp)
            .map(|seconds| seconds as i64)
            .ok_or_else(|| anyhow::anyhow!("invalid chapter timestamp in {line:?}"))?;
        if seconds <= previous {
            bail!("chapter timestamps must increase, but {line:?} does not");
        }
        previous = seconds;
        let title = title.trim();
        if *line != policy.first_chapter
            && (words(title) < policy.chapter_title_min_words as usize
                || words(title) > policy.chapter_title_max_words as usize)
        {
            bail!(
                "chapter title must contain {}-{} words: {title:?}",
                policy.chapter_title_min_words,
                policy.chapter_title_max_words
            );
        }
    }
    if contains_emoji(text) {
        bail!("chapters contain emoji");
    }

    // A model that saw only part of the transcript still emits a well-formed
    // list, so shape alone cannot detect a truncated prompt. Comparing the last
    // chapter against the transcript end is what turns that into a failure.
    if let Some(end) = transcript_end {
        if end == 0 {
            bail!("transcript end must be greater than zero");
        }
        let covered = previous.max(0) as f64 / end as f64;
        if covered < policy.chapter_coverage {
            bail!(
                "chapters reach {} of a {} transcript ({:.0}%, minimum {:.0}%); the model likely saw a truncated transcript",
                transcript::format_timestamp(previous.max(0) as f64),
                transcript::format_timestamp(end as f64),
                covered * 100.0,
                policy.chapter_coverage * 100.0
            );
        }
    }
    Ok(())
}

/// Attach the offending line to a shape failure.
trait LineContext<T> {
    fn with_context_line(self, line: &str) -> Result<T>;
}

impl<T> LineContext<T> for Option<T> {
    fn with_context_line(self, line: &str) -> Result<T> {
        self.ok_or_else(|| anyhow::anyhow!("invalid chapter line: {line:?}"))
    }
}

fn tags(text: &str, policy: &MetadataPolicy) -> Result<()> {
    let line = text.trim();
    if line.contains('\n') {
        bail!("tags must be a single line");
    }
    if line.chars().count() > policy.tags_max_characters as usize {
        bail!(
            "tags must be {} characters or fewer, got {}",
            policy.tags_max_characters,
            line.chars().count()
        );
    }
    if line.contains(", ") || line.contains('#') {
        bail!("tags must be comma-separated without spaces after commas or hashtags");
    }
    let entries = line.split(',').collect::<Vec<_>>();
    if entries.len() < policy.tags_min as usize
        || entries.len() > policy.tags_max as usize
        || entries.iter().any(|tag| tag.trim().is_empty())
    {
        bail!(
            "tags must contain {}-{} non-empty entries, got {}",
            policy.tags_min,
            policy.tags_max,
            entries.len()
        );
    }
    if let Some(required) = &policy.tags_required_final
        && !entries
            .last()
            .is_some_and(|tag| tag.eq_ignore_ascii_case(required))
    {
        bail!("last tag must be \"{required}\"");
    }
    if contains_emoji(text) {
        bail!("tags contain emoji");
    }
    Ok(())
}

fn thumbnail_quote(text: &str, policy: &MetadataPolicy) -> Result<()> {
    let candidates = non_empty_lines(text);
    if candidates.len() != policy.thumbnail_quote_count as usize {
        bail!(
            "expected exactly {} quote candidates, got {}",
            policy.thumbnail_quote_count,
            candidates.len()
        );
    }
    for candidate in &candidates {
        let count = words(candidate);
        if count < policy.thumbnail_quote_min_words as usize
            || count > policy.thumbnail_quote_max_words as usize
        {
            bail!(
                "thumbnail quote must contain {}-{} words: {candidate:?}",
                policy.thumbnail_quote_min_words,
                policy.thumbnail_quote_max_words
            );
        }
        if candidate.starts_with(|character: char| character.is_ascii_digit())
            || candidate.contains('!')
        {
            bail!("thumbnail quote must not be numbered or use exclamation marks: {candidate:?}");
        }
    }
    if contains_emoji(text) {
        bail!("thumbnail quotes contain emoji");
    }
    Ok(())
}

fn description(text: &str, policy: &MetadataPolicy) -> Result<()> {
    if text.trim().is_empty() {
        bail!("description is empty");
    }
    if text.chars().count() > policy.description_max_characters as usize {
        bail!(
            "description exceeds the {}-character limit",
            policy.description_max_characters
        );
    }
    for heading in &policy.description_required_headings {
        if !text.contains(heading.as_str()) {
            bail!("description is missing required heading: {heading}");
        }
    }
    if contains_emoji(text) {
        bail!("description contains emoji");
    }
    Ok(())
}

fn social(task: &str, text: &str, policy: &MetadataPolicy) -> Result<()> {
    if text.trim().is_empty() {
        bail!("social draft is empty");
    }
    if contains_emoji(text) {
        bail!("social draft contains emoji");
    }
    let Some(limits) = policy.social.get(task) else {
        return Ok(());
    };
    let urls = text.matches("http://").count() + text.matches("https://").count();

    if let Some(max) = limits.max_characters
        && text.chars().count() > max as usize
    {
        bail!(
            "{task} must be {max} characters or fewer, got {}",
            text.chars().count()
        );
    }
    if let (Some(min), Some(max)) = (limits.min_words, limits.max_words) {
        let count = words(text);
        if count < min as usize || count > max as usize {
            bail!("{task} must contain {min}-{max} words, got {count}");
        }
    }
    if let Some(expected) = limits.urls
        && urls != expected as usize
    {
        bail!("{task} must contain exactly {expected} URL(s), got {urls}",);
    }
    if let Some(required) = &limits.must_contain
        && !text.contains(required.as_str())
    {
        bail!("{task} is missing required text: {required:?}");
    }
    Ok(())
}

fn guest_research(text: &str) -> Result<()> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|error| anyhow::anyhow!("must be valid JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("guest research must be a JSON object"))?;
    const KEYS: &[&str] = &[
        "name",
        "organisation",
        "linkedin",
        "bluesky",
        "website",
        "bio",
    ];
    let mut found = object.keys().map(String::as_str).collect::<Vec<_>>();
    found.sort_unstable();
    let mut expected = KEYS.to_vec();
    expected.sort_unstable();
    if found != expected {
        bail!(
            "guest research must contain exactly: {}",
            expected.join(", ")
        );
    }
    if !object.values().all(serde_json::Value::is_string) {
        bail!("guest research values must all be strings");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::SocialLimits;
    use std::collections::BTreeMap;

    fn policy() -> MetadataPolicy {
        MetadataPolicy {
            tags_required_final: Some("everything digital health".to_owned()),
            description_required_headings: vec![
                "CHAPTERS".to_owned(),
                "LINKS MENTIONED".to_owned(),
            ],
            social: BTreeMap::from([
                (
                    "bluesky-draft".to_owned(),
                    SocialLimits {
                        max_characters: Some(280),
                        urls: Some(0),
                        ..SocialLimits::default()
                    },
                ),
                (
                    "linkedin-draft".to_owned(),
                    SocialLimits {
                        min_words: Some(150),
                        max_words: Some(300),
                        must_contain: Some("Full episode linked in the comments.".to_owned()),
                        ..SocialLimits::default()
                    },
                ),
            ]),
            ..MetadataPolicy::default()
        }
    }

    fn good_chapters() -> String {
        let mut text = String::from("0:00 Introduction\n");
        for index in 1..7 {
            text.push_str(&format!("{index}:00 A chapter about things\n"));
        }
        text
    }

    #[test]
    fn accepts_a_well_formed_chapter_list() {
        check("chapters", &good_chapters(), &policy(), Some(400)).unwrap();
    }

    #[test]
    fn rejects_chapters_that_stop_early() {
        // The real failure: a well-formed list covering half the episode.
        let error = check("chapters", &good_chapters(), &policy(), Some(3000))
            .unwrap_err()
            .to_string();
        assert!(error.contains("truncated transcript"), "{error}");
    }

    #[test]
    fn rejects_chapters_that_go_backwards_or_start_wrong() {
        let backwards = "0:00 Introduction\n5:00 A chapter about things\n2:00 Another chapter here\n1:00 A third chapter now\n6:00 A fourth chapter here\n7:00 A fifth chapter here\n";
        assert!(check("chapters", backwards, &policy(), None).is_err());
        let wrong_start = good_chapters().replace("0:00 Introduction", "0:01 Introduction");
        assert!(check("chapters", &wrong_start, &policy(), None).is_err());
    }

    #[test]
    fn enforces_the_channel_tag_contract() {
        let mut tags = (0..11)
            .map(|index| format!("tag{index}"))
            .collect::<Vec<_>>();
        tags.push("everything digital health".to_owned());
        check("tags", &tags.join(","), &policy(), None).unwrap();

        assert!(check("tags", "too,few,tags", &policy(), None).is_err());
        assert!(check("tags", &tags.join(", "), &policy(), None).is_err());
        let wrong_final = tags[..tags.len() - 1].join(",") + ",something else";
        assert!(check("tags", &wrong_final, &policy(), None).is_err());
    }

    #[test]
    fn a_channel_without_a_required_final_tag_does_not_get_one_imposed() {
        let mut policy = policy();
        policy.tags_required_final = None;
        let tags = (0..12).map(|i| format!("tag{i}")).collect::<Vec<_>>();
        check("tags", &tags.join(","), &policy, None).unwrap();
    }

    #[test]
    fn requires_the_configured_description_headings() {
        let policy = policy();
        check(
            "youtube-description",
            "CHAPTERS\nLINKS MENTIONED\n",
            &policy,
            None,
        )
        .unwrap();
        assert!(check("youtube-description", "CHAPTERS only", &policy, None).is_err());
    }

    #[test]
    fn rejects_emoji_anywhere() {
        let policy = policy();
        let chapters = good_chapters().replace("Introduction", "Introduction 🎉");
        assert!(check("chapters", &chapters, &policy, None).is_err());
        assert!(check("bluesky-draft", "A post 🚀", &policy, None).is_err());
    }

    #[test]
    fn enforces_platform_limits_from_config() {
        let policy = policy();
        assert!(check("bluesky-draft", &"a ".repeat(200), &policy, None).is_err());
        check(
            "bluesky-draft",
            "A short post about the episode.",
            &policy,
            None,
        )
        .unwrap();
        assert!(
            check("bluesky-draft", "See https://example.com", &policy, None).is_err(),
            "a URL in a Bluesky draft should fail"
        );
    }

    #[test]
    fn checks_guest_research_shape() {
        let good = r#"{"name":"A","organisation":"B","linkedin":"C","bluesky":"D","website":"E","bio":"F"}"#;
        check("guest-research", good, &policy(), None).unwrap();
        assert!(check("guest-research", "not json", &policy(), None).is_err());
        assert!(check("guest-research", r#"{"name":"A"}"#, &policy(), None).is_err());
        assert!(
            check("guest-research", r#"{"name":1,"organisation":"B","linkedin":"C","bluesky":"D","website":"E","bio":"F"}"#, &policy(), None).is_err()
        );
    }

    #[test]
    fn counts_words_the_way_a_human_would() {
        assert_eq!(words("three little words"), 3);
        assert_eq!(words("it's a well-known thing"), 4);
        assert_eq!(words("  "), 0);
    }
}
