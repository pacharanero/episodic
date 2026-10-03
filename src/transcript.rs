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

use anyhow::{Context, Result, bail};
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

/// Longest cue text still treated as a candidate repeated filler phrase.
const SHORT_CUE_CHARS: usize = 40;

/// How many trailing cues start at or after the media duration.
///
/// A cue beginning after the file has ended cannot be describing anything in
/// it, so this needs no further evidence. Cues are in file order, so the
/// offenders are always a suffix.
pub fn beyond_duration(cues: &[Cue], duration: f64) -> usize {
    cues.iter()
        .rev()
        .take_while(|cue| cue.start >= duration)
        .count()
}

/// How many trailing cues form a run of repeated short phrases.
///
/// Whisper's trailing-silence failure mode repeats a short phrase; the
/// observed case was eight `Bye.` followed by two `Thank you.`. A cue joins
/// the run only when it is short *and* identical to its neighbour on either
/// side, so a single distinct closing sentence is never swept up.
///
/// This is suspicion, not proof: a phrase genuinely repeated on camera looks
/// the same. The caller confirms the span is silent before removing anything.
/// A run covering every cue is reported as no run at all, because an empty
/// transcript is worse than a bad one and the two cases are indistinguishable
/// from the text alone.
pub fn repeated_tail(cues: &[Cue]) -> usize {
    let mut count = 0;
    for index in (0..cues.len()).rev() {
        let text = cues[index].text.trim();
        if text.chars().count() > SHORT_CUE_CHARS {
            break;
        }
        let matches_before = index > 0 && cues[index - 1].text.trim() == text;
        let matches_after = index + 1 < cues.len() && cues[index + 1].text.trim() == text;
        if !matches_before && !matches_after {
            break;
        }
        count += 1;
    }
    if count == cues.len() { 0 } else { count }
}

/// Keep the first `keep` cue blocks of an SRT or WebVTT document.
///
/// The kept bytes are preserved exactly, including the `WEBVTT` header and SRT
/// cue numbering, which stays correct because only a suffix is removed.
pub fn truncate_cue_blocks(text: &str, keep: usize) -> String {
    let starts = cue_block_starts(text);
    match starts.get(keep) {
        None => text.to_owned(),
        Some(&cut) => text[..cut].to_owned(),
    }
}

/// Byte offsets where each cue block begins. Blocks are separated by blank
/// lines; one without a timestamp line, such as the `WEBVTT` header, is not a
/// cue block and so is never counted or cut.
fn cue_block_starts(text: &str) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut offset = 0;
    let mut block_start = 0;
    let mut in_block = false;
    let mut has_timestamp = false;

    for line in text.split_inclusive('\n') {
        if line.trim().is_empty() {
            if in_block && has_timestamp {
                starts.push(block_start);
            }
            in_block = false;
            has_timestamp = false;
        } else {
            if !in_block {
                block_start = offset;
                in_block = true;
            }
            has_timestamp |= line.contains("-->");
        }
        offset += line.len();
    }
    if in_block && has_timestamp {
        starts.push(block_start);
    }
    starts
}

/// Keep the first `keep` records of a line-per-cue document, after `header`
/// leading lines. Whisper writes one line per segment to `.txt`, and a header
/// plus one line per segment to `.tsv`.
pub fn truncate_lines(text: &str, keep: usize, header: usize) -> String {
    let mut result = String::new();
    let mut records = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if index >= header {
            if line.trim().is_empty() {
                continue;
            }
            if records == keep {
                break;
            }
            records += 1;
        }
        result.push_str(line);
    }
    result
}

/// Keep the first `keep` segments of Whisper's JSON, leaving every other
/// field, including each segment's own extra fields, as Whisper wrote them.
pub fn truncate_json(text: &str, keep: usize) -> Result<String> {
    let mut document: serde_json::Value =
        serde_json::from_str(text).context("whisper produced JSON that cannot be parsed")?;
    let Some(segments) = document
        .get_mut("segments")
        .and_then(|value| value.as_array_mut())
    else {
        bail!("whisper JSON has no segments array");
    };
    segments.truncate(keep);
    let joined = segments
        .iter()
        .filter_map(|segment| segment.get("text").and_then(|text| text.as_str()))
        .collect::<Vec<_>>()
        .join("");
    if let Some(whole) = document.get_mut("text") {
        *whole = serde_json::Value::String(joined);
    }
    serde_json::to_string(&document).context("could not rewrite whisper JSON")
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

    /// The observed failure: ten trailing cues over a silent tail, the last
    /// ending 29 seconds past the end of the media file.
    fn hallucinated_tail() -> Vec<Cue> {
        let mut cues = vec![Cue {
            start: 1240.0,
            end: 1244.74,
            text: "and that is where the evidence runs out".to_owned(),
        }];
        for index in 0..8 {
            let start = 1255.0 + f64::from(index) * 5.0;
            cues.push(Cue {
                start,
                end: start + 5.0,
                text: "Bye.".to_owned(),
            });
        }
        cues.push(Cue {
            start: 1295.0,
            end: 1325.0,
            text: "Thank you.".to_owned(),
        });
        cues.push(Cue {
            start: 1305.88,
            end: 1334.88,
            text: "Thank you.".to_owned(),
        });
        cues
    }

    #[test]
    fn counts_cues_that_begin_after_the_media_ends() {
        assert_eq!(beyond_duration(&hallucinated_tail(), 1305.60), 1);
        assert_eq!(beyond_duration(&parse(SAMPLE), 1305.60), 0);
    }

    #[test]
    fn finds_the_repeated_run_but_not_the_last_real_sentence() {
        let cues = hallucinated_tail();
        assert_eq!(repeated_tail(&cues), 10);
        assert_eq!(
            cues[cues.len() - 11].text,
            "and that is where the evidence runs out",
            "the run must stop at real speech"
        );
    }

    #[test]
    fn a_single_distinct_closing_cue_is_not_a_repeated_run() {
        let mut cues = parse(SAMPLE);
        cues.push(Cue {
            start: 120.0,
            end: 122.0,
            text: "Thanks for watching.".to_owned(),
        });
        assert_eq!(repeated_tail(&cues), 0);
    }

    #[test]
    fn a_transcript_that_is_entirely_repeats_is_left_alone() {
        // Indistinguishable from a legitimately repetitive clip, and an empty
        // transcript is worse than a suspicious one.
        let cues = vec![
            Cue {
                start: 0.0,
                end: 1.0,
                text: "Bye.".to_owned(),
            },
            Cue {
                start: 1.0,
                end: 2.0,
                text: "Bye.".to_owned(),
            },
        ];
        assert_eq!(repeated_tail(&cues), 0);
    }

    #[test]
    fn truncating_srt_keeps_numbering_and_exact_bytes() {
        let kept = truncate_cue_blocks(SAMPLE, 2);
        assert_eq!(
            kept,
            "1\n00:00:00,000 --> 00:00:04,000\nHello and welcome\n\n2\n00:00:05,000 --> 00:00:09,500\nto the show\n\n"
        );
        assert!(SAMPLE.starts_with(&kept), "kept bytes must be unchanged");
        assert_eq!(truncate_cue_blocks(SAMPLE, 9), SAMPLE);
    }

    #[test]
    fn truncating_vtt_preserves_the_header() {
        let vtt = "WEBVTT\n\n00:00:00.000 --> 00:00:04.000\nHello\n\n00:00:05.000 --> 00:00:09.500\nthere\n\n";
        let kept = truncate_cue_blocks(vtt, 1);
        assert!(kept.starts_with("WEBVTT\n\n"));
        assert!(!kept.contains("there"));
    }

    #[test]
    fn truncating_lines_respects_a_header() {
        let tsv = "start\tend\ttext\n0\t4000\tHello\n5000\t9500\tthere\n";
        assert_eq!(
            truncate_lines(tsv, 1, 1),
            "start\tend\ttext\n0\t4000\tHello\n"
        );
        assert_eq!(truncate_lines("Hello\nthere\n", 1, 0), "Hello\n");
    }

    #[test]
    fn truncating_json_keeps_whisper_fields_and_rebuilds_the_whole_text() {
        let json = r#"{"text":" Hello there","segments":[{"id":0,"seek":0,"start":0.0,"end":4.0,"text":" Hello","tokens":[1,2]},{"id":1,"seek":0,"start":5.0,"end":9.5,"text":" there","tokens":[3]}],"language":"en"}"#;
        let kept: serde_json::Value =
            serde_json::from_str(&truncate_json(json, 1).unwrap()).unwrap();
        assert_eq!(kept["text"], " Hello");
        assert_eq!(kept["language"], "en");
        assert_eq!(kept["segments"].as_array().unwrap().len(), 1);
        assert_eq!(
            kept["segments"][0]["tokens"],
            serde_json::json!([1, 2]),
            "per-segment extras must survive"
        );
        assert!(truncate_json("not json", 1).is_err());
    }
}
