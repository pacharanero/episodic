// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reading SRT transcripts and condensing them for prompts.
//!
//! Whisper's segmentation is not stable between episodes: usually it emits
//! phrase-level cues, sometimes word-level ones. In SRT every cue costs roughly
//! 25 tokens of index and timestamp scaffolding to carry as little as two
//! tokens of speech, so a word-level transcript can be three times the prompt
//! size of a phrase-level transcript of the same episode, with the real
//! timestamps buried in repetitive noise.
//!
//! The observed failure, on a 25 minute episode with 696 word-level cues, was
//! a well-formed chapter list that stopped at 14:00 and used evenly spaced
//! invented timestamps. Condensing into fixed-length blocks makes prompt size
//! track episode duration rather than cue count, and fixed it reproducibly.

use anyhow::{Result, bail};
use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Parse cues in file order.
///
/// Blocks are split on blank lines rather than by counting them, which
/// tolerates the optional index line, multi-line cue text, and stray
/// whitespace.
pub fn parse(text: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    for block in text.trim().split("\n\n") {
        let lines = block
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        let Some(index) = lines.iter().position(|line| line.contains("-->")) else {
            continue;
        };
        let Some((start, end)) = parse_range(lines[index]) else {
            continue;
        };
        let content = lines[index + 1..].join(" ").trim().to_owned();
        if !content.is_empty() {
            cues.push(Cue {
                start,
                end,
                text: content,
            });
        }
    }
    cues
}

fn parse_range(line: &str) -> Option<(f64, f64)> {
    let (left, right) = line.split_once("-->")?;
    Some((
        parse_timestamp(left.trim())?,
        parse_timestamp(right.trim())?,
    ))
}

/// `HH:MM:SS,mmm` or `HH:MM:SS.mmm`, and the same without hours.
pub fn parse_timestamp(value: &str) -> Option<f64> {
    let value = value.split_whitespace().next()?;
    let (clock, fraction) = match value.split_once([',', '.']) {
        Some((clock, fraction)) => (clock, fraction),
        None => (value, "0"),
    };
    let parts = clock.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [hours, minutes, seconds] => (hours.parse::<u64>().ok()?, *minutes, *seconds),
        [minutes, seconds] => (0, *minutes, *seconds),
        _ => return None,
    };
    let millis = format!("{fraction:0<3}")[..3].parse::<u64>().ok()?;
    Some(
        hours as f64 * 3600.0
            + minutes.parse::<u64>().ok()? as f64 * 60.0
            + seconds.parse::<u64>().ok()? as f64
            + millis as f64 / 1000.0,
    )
}

/// When the last cue ends, in whole seconds.
pub fn end_seconds(cues: &[Cue]) -> u64 {
    cues.iter().map(|cue| cue.end).fold(0.0, f64::max) as u64
}

/// One line per block: `[M:SS] words spoken during this block`.
pub fn condense(cues: &[Cue], block_seconds: f64) -> String {
    let mut digest = String::new();
    let mut block_start: Option<f64> = None;
    let mut spoken: Vec<&str> = Vec::new();

    for cue in cues {
        match block_start {
            None => block_start = Some(cue.start),
            Some(start) if cue.start - start >= block_seconds => {
                let _ = writeln!(digest, "[{}] {}", format_timestamp(start), spoken.join(" "));
                block_start = Some(cue.start);
                spoken.clear();
            }
            Some(_) => {}
        }
        spoken.push(&cue.text);
    }
    if let Some(start) = block_start
        && !spoken.is_empty()
    {
        let _ = writeln!(digest, "[{}] {}", format_timestamp(start), spoken.join(" "));
    }
    digest
}

pub fn format_timestamp(seconds: f64) -> String {
    let total = seconds as u64;
    let (hours, remainder) = (total / 3600, total % 3600);
    let (minutes, seconds) = (remainder / 60, remainder % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Read and condense a transcript, returning the digest and its end time.
pub fn digest(text: &str, block_seconds: f64) -> Result<(String, u64)> {
    if block_seconds <= 0.0 {
        bail!("block seconds must be greater than zero");
    }
    let cues = parse(text);
    if cues.is_empty() {
        bail!("no SRT cues found; is this a Whisper transcript?");
    }
    Ok((condense(&cues, block_seconds), end_seconds(&cues)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "1\n00:00:00,000 --> 00:00:04,000\nHello and welcome\n\n2\n00:00:05,000 --> 00:00:09,500\nto the show\n\n3\n00:01:00,000 --> 00:01:04,000\nA later thought\n";

    #[test]
    fn parses_cues_and_the_end_time() {
        let cues = parse(SAMPLE);
        assert_eq!(cues.len(), 3);
        assert_eq!(cues[0].text, "Hello and welcome");
        assert_eq!(cues[2].start, 60.0);
        assert_eq!(end_seconds(&cues), 64);
    }

    #[test]
    fn tolerates_missing_index_lines_and_multi_line_cues() {
        let cues = parse("00:00:01,000 --> 00:00:02,000\nfirst line\nsecond line\n");
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "first line second line");
    }

    #[test]
    fn ignores_blocks_without_a_timestamp() {
        assert!(parse("WEBVTT\n\nnot a cue\n").is_empty());
    }

    #[test]
    fn groups_speech_into_blocks_by_elapsed_time() {
        let digest = condense(&parse(SAMPLE), 30.0);
        assert_eq!(
            digest,
            "[0:00] Hello and welcome to the show\n[1:00] A later thought\n"
        );
    }

    #[test]
    fn makes_prompt_size_track_duration_not_cue_count() {
        // The word-level case that caused invented timestamps: many tiny cues
        // covering the same minute must condense to the same few blocks as a
        // handful of phrase-level ones.
        let mut srt = String::new();
        for index in 0..600 {
            let start = index as f64 * 0.1;
            let _ = writeln!(
                srt,
                "{}\n00:00:{:02},{:03} --> 00:00:{:02},{:03}\nword\n",
                index + 1,
                start as u64,
                ((start % 1.0) * 1000.0) as u64,
                start as u64,
                ((start % 1.0) * 1000.0) as u64
            );
        }
        let blocks = condense(&parse(&srt), 30.0).lines().count();
        assert_eq!(blocks, 2, "600 word cues over a minute should be 2 blocks");
    }

    #[test]
    fn formats_timestamps_the_way_youtube_expects() {
        assert_eq!(format_timestamp(0.0), "0:00");
        assert_eq!(format_timestamp(65.4), "1:05");
        assert_eq!(format_timestamp(3725.0), "1:02:05");
    }

    #[test]
    fn parses_both_comma_and_dot_fractions() {
        assert_eq!(parse_timestamp("00:00:01,500"), Some(1.5));
        assert_eq!(parse_timestamp("00:00:01.500"), Some(1.5));
        assert_eq!(parse_timestamp("01:00"), Some(60.0));
        assert_eq!(parse_timestamp("nonsense"), None);
    }

    #[test]
    fn refuses_input_that_is_not_a_transcript() {
        assert!(digest("just some prose", 30.0).is_err());
        assert!(digest(SAMPLE, 0.0).is_err());
    }
}
