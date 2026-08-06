// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Lossless read and write access to `<base>.episode-data.md`.
//!
//! The document is the authoritative production record for an episode and is
//! mostly human-authored. Everything here exists to change the few labelled
//! values `eps` owns while leaving every other byte exactly as it was found:
//! no template rebuild, no reordering, no reflowing, no normalising of line
//! endings or whitespace.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// One source line and the exact terminator that followed it, so a document
/// with CRLF, mixed, or absent final line endings rebuilds byte-for-byte.
#[derive(Clone, Debug)]
struct Line {
    text: String,
    terminator: String,
}

#[derive(Debug)]
pub struct Document {
    path: PathBuf,
    lines: Vec<Line>,
}

/// One applied or proposed edit, reported identically in both cases so a
/// dry run shows exactly what an apply would do.
#[derive(Debug, Serialize)]
pub struct Change {
    /// Enclosing heading used to disambiguate the field, when one was needed.
    pub section: Option<String>,
    pub label: String,
    pub from: String,
    pub to: String,
    /// One-indexed line in the source document.
    pub line: usize,
}

impl Change {
    pub fn as_text(&self) -> String {
        let label = match &self.section {
            Some(section) => format!("{section} / {}", self.label),
            None => self.label.clone(),
        };
        if self.from.contains('\n') || self.to.contains('\n') {
            return format!(
                "  {label}: replace {} lines with {} lines",
                self.from.lines().count(),
                self.to.lines().count()
            );
        }
        format!(
            "  {label}: {} -> {}",
            display(&self.from),
            display(&self.to)
        )
    }
}

fn display(value: &str) -> String {
    if value.trim().is_empty() {
        "(empty)".to_owned()
    } else {
        value.to_owned()
    }
}

impl Document {
    pub fn read(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("could not read episode metadata {}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            lines: split_lines(&source),
        })
    }

    /// Rebuild the source. Byte-identical to the input when nothing changed.
    pub fn render(&self) -> String {
        let mut rendered = String::new();
        for line in &self.lines {
            rendered.push_str(&line.text);
            rendered.push_str(&line.terminator);
        }
        rendered
    }

    /// Set a managed `- **Label**: value` field.
    ///
    /// Returns `None` when the field already holds this value, so repeating a
    /// reconciliation is a no-op rather than a rewrite. A field holding a real
    /// value is never silently replaced: that requires `force`, which is where
    /// human approval or an idempotency match belongs.
    pub fn set_field(
        &mut self,
        label: &str,
        section: Option<&str>,
        value: &str,
        force: bool,
    ) -> Result<Option<Change>> {
        let index = self.locate_field(label, section)?;
        let (value_start, current) = field_value(&self.lines[index].text)
            .expect("locate_field only returns lines that parse as fields");
        let current = current.to_owned();

        // Legacy documents wrap some values in code spans, so `` `abc123` ``
        // and `abc123` are the same recorded fact. Treating them as equal keeps
        // a repeated reconciliation a genuine no-op instead of a forced rewrite.
        if current == value || current.trim_matches('`') == value.trim_matches('`') {
            return Ok(None);
        }
        if !is_placeholder(&current) && !force {
            bail!(
                "{label} already holds '{current}' in {}; pass --force to replace a value that is not a placeholder",
                self.path.display()
            );
        }

        let line = &mut self.lines[index];
        let mut text = line.text[..value_start].to_owned();
        if !value.is_empty() && !text.ends_with(' ') {
            text.push(' ');
        }
        text.push_str(value);
        line.text = text;

        Ok(Some(Change {
            section: section.map(str::to_owned),
            label: label.to_owned(),
            from: current,
            to: value.to_owned(),
            line: index + 1,
        }))
    }

    /// Replace the fenced draft block under `#### <heading>` within a platform
    /// section, leaving the fence markers and all surrounding prose untouched.
    pub fn set_draft(
        &mut self,
        platform: &str,
        heading: &str,
        text: &str,
    ) -> Result<Option<Change>> {
        let heading_index = self.locate_heading(heading, Some(platform))?;
        let (open, close) = self.locate_fence(heading_index)?;

        let current = self.lines[open + 1..close]
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let replacement = text.trim_end_matches('\n');
        if current == replacement {
            return Ok(None);
        }

        // Reuse the terminator already in use inside the block so a CRLF
        // document stays CRLF.
        let terminator = self
            .lines
            .get(open + 1)
            .filter(|line| !line.terminator.is_empty())
            .map(|line| line.terminator.clone())
            .unwrap_or_else(|| self.lines[open].terminator.clone());
        let replacement_lines = replacement
            .split('\n')
            .map(|text| Line {
                text: text.to_owned(),
                terminator: terminator.clone(),
            })
            .collect::<Vec<_>>();
        self.lines.splice(open + 1..close, replacement_lines);

        Ok(Some(Change {
            section: Some(platform.to_owned()),
            label: heading.to_owned(),
            from: current,
            to: replacement.to_owned(),
            line: open + 2,
        }))
    }

    /// Replace the file with the current content, atomically and in place.
    ///
    /// The rendered document is written to a temporary file in the same
    /// directory, flushed, given the original file's permissions, and only then
    /// renamed over the target. A crash or a full disk leaves the original
    /// intact.
    pub fn write(&self) -> Result<()> {
        let directory = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .context("episode metadata filename is not valid UTF-8")?;

        let mut temporary = tempfile::Builder::new()
            .prefix(&format!(".{name}."))
            .tempfile_in(directory)
            .with_context(|| format!("could not stage a write in {}", directory.display()))?;
        temporary.write_all(self.render().as_bytes())?;
        temporary.as_file().sync_all()?;

        if let Ok(metadata) = fs::metadata(&self.path) {
            let _ = fs::set_permissions(temporary.path(), metadata.permissions());
        }
        temporary
            .persist(&self.path)
            .with_context(|| format!("could not replace {}", self.path.display()))?;
        Ok(())
    }

    /// Find the single line defining `label`, optionally within a section.
    ///
    /// Ambiguity is an error rather than a first match: `Post URL` exists under
    /// every social platform, and guessing would write to the wrong one.
    fn locate_field(&self, label: &str, section: Option<&str>) -> Result<usize> {
        let matches = self
            .scan()
            .filter(|(_, _, line)| field_label(&line.text).is_some_and(|found| found == label))
            .filter(|(_, headings, _)| within(headings, section))
            .map(|(index, headings, _)| (index, headings))
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [(index, _)] => Ok(*index),
            [] => match section {
                Some(section) => bail!(
                    "no '{label}' field under section '{section}' in {}",
                    self.path.display()
                ),
                None => bail!("no '{label}' field in {}", self.path.display()),
            },
            _ => {
                let sections = matches
                    .iter()
                    .map(|(_, headings)| headings.last().cloned().unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(", ");
                bail!("'{label}' appears in several sections ({sections}); pass --section")
            }
        }
    }

    fn locate_heading(&self, heading: &str, section: Option<&str>) -> Result<usize> {
        self.scan()
            .find(|(_, headings, line)| {
                parse_heading(&line.text).is_some_and(|(_, title)| title == heading)
                    && within(headings, section)
            })
            .map(|(index, _, _)| index)
            .with_context(|| match section {
                Some(section) => format!(
                    "no '{heading}' block under section '{section}' in {}",
                    self.path.display()
                ),
                None => format!("no '{heading}' block in {}", self.path.display()),
            })
    }

    /// The opening and closing fence lines of the first code block after
    /// `start`, searching only up to the next heading so a platform with no
    /// draft block does not capture the following platform's.
    fn locate_fence(&self, start: usize) -> Result<(usize, usize)> {
        let mut open = None;
        for (index, line) in self.lines.iter().enumerate().skip(start + 1) {
            if open.is_none() && parse_heading(&line.text).is_some() {
                break;
            }
            match (open, fence_width(&line.text)) {
                (None, Some(_)) => open = Some(index),
                (Some(start), Some(width))
                    if width >= fence_width(&self.lines[start].text).unwrap_or(3) =>
                {
                    return Ok((start, index));
                }
                _ => {}
            }
        }
        bail!(
            "no closed code block follows line {} in {}",
            start + 1,
            self.path.display()
        )
    }

    /// Walk the document outside fenced blocks, yielding each line with the
    /// stack of headings enclosing it. Fenced content is skipped so a draft
    /// containing `- **Post URL**:` is never mistaken for a managed field.
    fn scan(&self) -> impl Iterator<Item = (usize, Vec<String>, &Line)> {
        let mut headings: Vec<(usize, String)> = Vec::new();
        let mut fence: Option<usize> = None;
        self.lines
            .iter()
            .enumerate()
            .filter_map(move |(index, line)| {
                match (fence, fence_width(&line.text)) {
                    (None, Some(width)) => {
                        fence = Some(width);
                        return None;
                    }
                    (Some(open), Some(width)) if width >= open => {
                        fence = None;
                        return None;
                    }
                    (Some(_), _) => return None,
                    _ => {}
                }
                if let Some((level, title)) = parse_heading(&line.text) {
                    headings.retain(|(existing, _)| *existing < level);
                    headings.push((level, title));
                }
                let stack = headings.iter().map(|(_, title)| title.clone()).collect();
                Some((index, stack, line))
            })
    }
}

/// A section filter matches any enclosing heading, so `--section YouTube`
/// reaches a field nested under `### Video`.
fn within(headings: &[String], section: Option<&str>) -> bool {
    match section {
        None => true,
        Some(section) => headings
            .iter()
            .any(|heading| heading.eq_ignore_ascii_case(section)),
    }
}

/// An unset field: empty, or a `[To be added]` style placeholder.
pub fn is_placeholder(value: &str) -> bool {
    let value = value.trim();
    value.is_empty() || value.starts_with('[')
}

fn split_lines(source: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut rest = source;
    while !rest.is_empty() {
        let (line, remainder) = match rest.find('\n') {
            Some(index) => rest.split_at(index + 1),
            None => (rest, ""),
        };
        let text = line.trim_end_matches('\n').trim_end_matches('\r');
        lines.push(Line {
            text: text.to_owned(),
            terminator: line[text.len()..].to_owned(),
        });
        rest = remainder;
    }
    lines
}

fn parse_heading(text: &str) -> Option<(usize, String)> {
    let level = text.bytes().take_while(|byte| *byte == b'#').count();
    if level == 0 {
        return None;
    }
    let title = text[level..].trim();
    if title.is_empty() {
        return None;
    }
    Some((level, title.to_owned()))
}

/// The backtick count of a fence line, or `None` if the line is not a fence.
fn fence_width(text: &str) -> Option<usize> {
    let width = text.trim_start().bytes().take_while(|b| *b == b'`').count();
    (width >= 3).then_some(width)
}

fn field_label(text: &str) -> Option<&str> {
    let (label, _) = text.trim_start().strip_prefix("- **")?.split_once("**:")?;
    Some(label.trim())
}

/// The byte offset where the value starts and the value itself, so a rewrite
/// preserves the original indentation, label, and separator spacing exactly.
fn field_value(text: &str) -> Option<(usize, &str)> {
    let indent = text.len() - text.trim_start().len();
    let rest = &text[indent..];
    let separator = rest.find("**:")? + "**:".len();
    let value = &rest[separator..];
    let leading = value.len() - value.trim_start().len();
    Some((indent + separator + leading, value.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANONICAL: &str =
        include_str!("../tests/fixtures/episode-data/canonical.episode-data.md");

    fn document(source: &str) -> Document {
        Document {
            path: PathBuf::from("fixture.episode-data.md"),
            lines: split_lines(source),
        }
    }

    #[test]
    fn rebuilds_source_byte_for_byte() {
        assert_eq!(document(CANONICAL).render(), CANONICAL);
        for source in ["", "no trailing newline", "crlf\r\nmixed\nends\r\n", "\n\n"] {
            assert_eq!(
                document(source).render(),
                source,
                "round trip of {source:?}"
            );
        }
    }

    #[test]
    fn sets_a_placeholder_field_and_leaves_every_other_line_alone() {
        let mut doc = document(CANONICAL);
        let change = doc
            .set_field("Video ID", None, "dQw4w9WgXcQ", false)
            .unwrap()
            .unwrap();
        assert_eq!(change.from, "[To be added]");
        assert_eq!(change.to, "dQw4w9WgXcQ");

        let before = CANONICAL.lines().collect::<Vec<_>>();
        let after = doc.render();
        let after = after.lines().collect::<Vec<_>>();
        assert_eq!(before.len(), after.len());
        let differing = before
            .iter()
            .zip(&after)
            .filter(|(before, after)| before != after)
            .count();
        assert_eq!(differing, 1);
        assert!(after.contains(&"- **Video ID**: dQw4w9WgXcQ"));
    }

    #[test]
    fn setting_the_same_value_again_changes_nothing() {
        let mut doc = document(CANONICAL);
        doc.set_field("Video ID", None, "abc123", false).unwrap();
        let repeat = doc.set_field("Video ID", None, "abc123", false).unwrap();
        assert!(repeat.is_none());
    }

    #[test]
    fn refuses_to_overwrite_a_real_value_without_force() {
        let mut doc = document(CANONICAL);
        let error = doc
            .set_field("Title", None, "Something Else", false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("--force"), "unexpected error: {error}");
        assert_eq!(doc.render(), CANONICAL);

        assert!(
            doc.set_field("Title", None, "Something Else", true)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn requires_a_section_when_a_label_is_ambiguous() {
        let source = format!(
            "{CANONICAL}\n### Mastodon\n\n- **Post URL**: [To be added]\n- **Post Date**: [To be added]\n"
        );
        let mut doc = document(&source);
        let error = doc
            .set_field("Post URL", None, "https://example.invalid/1", false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("--section"), "unexpected error: {error}");

        let change = doc
            .set_field(
                "Post URL",
                Some("Mastodon"),
                "https://example.invalid/1",
                false,
            )
            .unwrap()
            .unwrap();
        assert_eq!(change.section.as_deref(), Some("Mastodon"));
        assert!(
            doc.render()
                .contains("Original human-reviewed Bluesky draft.")
        );
    }

    #[test]
    fn reports_an_unknown_field_instead_of_inventing_one() {
        let mut doc = document(CANONICAL);
        let error = doc
            .set_field("Invented Field", None, "value", false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("no 'Invented Field' field"));
        assert_eq!(doc.render(), CANONICAL);
    }

    #[test]
    fn replaces_draft_text_inside_its_fence() {
        let mut doc = document(CANONICAL);
        doc.set_draft("Bluesky", "Draft", "First line.\nSecond line.")
            .unwrap()
            .unwrap();
        let rendered = doc.render();
        assert!(rendered.contains("```text\nFirst line.\nSecond line.\n```"));
        assert!(!rendered.contains("Original human-reviewed Bluesky draft."));
        assert!(rendered.contains("## Custom Human Context"));
        assert!(rendered.contains("- **Post URL**: [To be added]"));
    }

    #[test]
    fn ignores_managed_labels_written_inside_a_draft() {
        let source = CANONICAL.replace(
            "Original human-reviewed Bluesky draft.",
            "- **Video ID**: not a real field",
        );
        let mut doc = document(&source);
        let change = doc
            .set_field("Video ID", None, "real", false)
            .unwrap()
            .unwrap();
        assert_eq!(change.from, "[To be added]");
        assert!(doc.render().contains("- **Video ID**: not a real field"));
    }

    #[test]
    fn treats_a_code_spanned_value_as_the_same_fact() {
        let source = "- **Video ID**: `fAUIq9rALTc`\n";
        let mut doc = document(source);
        assert!(
            doc.set_field("Video ID", None, "fAUIq9rALTc", false)
                .unwrap()
                .is_none()
        );
        assert_eq!(doc.render(), source);
    }

    /// Opt-in conformance run over a real episode corpus, which cannot be
    /// committed because it holds live IDs. Point `EPISODIC_CORPUS` at an
    /// episodes directory to prove every historical document shape round-trips.
    #[test]
    fn round_trips_a_real_corpus_when_one_is_supplied() {
        let Ok(root) = std::env::var("EPISODIC_CORPUS") else {
            return;
        };
        let mut checked = 0;
        let mut stack = vec![PathBuf::from(root)];
        while let Some(directory) = stack.pop() {
            for entry in fs::read_dir(&directory).unwrap().filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.to_string_lossy().ends_with(".episode-data.md") {
                    let source = fs::read_to_string(&path).unwrap();
                    assert_eq!(document(&source).render(), source, "{}", path.display());
                    checked += 1;
                }
            }
        }
        assert!(checked > 0, "no episode documents found");
        eprintln!("round-tripped {checked} real documents");
    }

    #[test]
    fn preserves_carriage_returns_when_setting_a_field() {
        let source = "## YouTube\r\n\r\n- **Video ID**: [To be added]\r\n";
        let mut doc = document(source);
        doc.set_field("Video ID", None, "abc", false).unwrap();
        assert_eq!(doc.render(), "## YouTube\r\n\r\n- **Video ID**: abc\r\n");
    }
}
