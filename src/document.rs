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

/// Text output is for reading; the full values are always in the structured
/// output, so a long value is shortened rather than wrapped across the screen.
fn display(value: &str) -> String {
    const LIMIT: usize = 60;
    let value = value.trim();
    if value.is_empty() {
        return "(empty)".to_owned();
    }
    if value.chars().count() <= LIMIT {
        return value.to_owned();
    }
    let kept = value.chars().take(LIMIT).collect::<String>();
    format!("{kept}... ({} characters)", value.chars().count())
}

impl Document {
    /// Parse an in-memory document. `path` is used only for error messages
    /// and by [`Document::write`].
    pub fn parse(path: &Path, source: &str) -> Self {
        Self {
            path: path.to_path_buf(),
            lines: split_lines(source),
        }
    }

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
        let change = self.replace_fence(heading_index, text.trim_end_matches('\n'))?;
        Ok(change.map(|change| Change {
            section: Some(platform.to_owned()),
            label: heading.to_owned(),
            ..change
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
        self.structure()
            .into_iter()
            .enumerate()
            .filter(|(_, info)| !info.fenced)
            .map(|(index, info)| (index, info.headings, &self.lines[index]))
    }

    /// Classify every line once: whether it is fenced, whether it opens a
    /// heading, and which headings enclose it.
    fn structure(&self) -> Vec<LineInfo> {
        let mut stack: Vec<(usize, String)> = Vec::new();
        let mut fence: Option<usize> = None;
        self.lines
            .iter()
            .map(|line| {
                let fenced = match (fence, fence_width(&line.text)) {
                    (None, Some(width)) => {
                        fence = Some(width);
                        true
                    }
                    (Some(open), Some(width)) if width >= open => {
                        fence = None;
                        true
                    }
                    (Some(_), _) => true,
                    _ => false,
                };
                let heading = (!fenced).then(|| parse_heading(&line.text)).flatten();
                if let Some((level, title)) = &heading {
                    stack.retain(|(existing, _)| existing < level);
                    stack.push((*level, title.clone()));
                }
                LineInfo {
                    fenced,
                    heading: heading.map(|(level, _)| level),
                    headings: stack.iter().map(|(_, title)| title.clone()).collect(),
                }
            })
            .collect()
    }

    /// The first heading matching any of `titles`, and the body lines beneath
    /// it up to the next heading of the same or higher level.
    fn locate_section(
        &self,
        titles: &[&str],
        info: &[LineInfo],
    ) -> Result<(usize, std::ops::Range<usize>)> {
        let heading = info
            .iter()
            .enumerate()
            .find(|(index, line)| {
                line.heading.is_some()
                    && parse_heading(&self.lines[*index].text).is_some_and(|(_, title)| {
                        titles.iter().any(|want| title.eq_ignore_ascii_case(want))
                    })
            })
            .map(|(index, _)| index)
            .with_context(|| {
                format!(
                    "no '{}' section in {}",
                    titles.join("' or '"),
                    self.path.display()
                )
            })?;

        let level = info[heading].heading.expect("located line is a heading");
        let end = (heading + 1..self.lines.len())
            .find(|index| info[*index].heading.is_some_and(|found| found <= level))
            .unwrap_or(self.lines.len());
        Ok((heading, heading + 1..end))
    }

    /// Tick the checkbox for one playlist and record how it was added.
    ///
    /// The search is confined to the playlist section because the same
    /// checkbox syntax is used by the manual YouTube Studio steps, where a
    /// stray tick would claim work nobody did. Playlists are matched by ID
    /// rather than by name: names have been edited over time, IDs have not.
    pub fn mark_playlist(&mut self, playlist: &str, note: Option<&str>) -> Result<Option<Change>> {
        let info = self.structure();
        let (_, body) = self.locate_section(PLAYLIST_HEADINGS, &info)?;

        let index = body
            .clone()
            .find(|index| {
                !info[*index].fenced
                    && checkbox(&self.lines[*index].text).is_some()
                    && self.lines[*index].text.contains(playlist)
            })
            .with_context(|| {
                format!(
                    "no playlist entry mentioning '{playlist}' in {}",
                    self.path.display()
                )
            })?;

        let (ticked, marker) = checkbox(&self.lines[index].text).expect("line is a checkbox");
        if ticked {
            return Ok(None);
        }

        let from = self.lines[index].text.clone();
        let mut text = from.clone();
        text.replace_range(marker..marker + 1, "x");
        if let Some(note) = note.filter(|note| !note.is_empty()) {
            text.push_str(&format!(" - {note}"));
        }
        self.lines[index].text = text.clone();

        Ok(Some(Change {
            section: Some(PLAYLIST_HEADINGS[0].to_owned()),
            label: playlist.to_owned(),
            from,
            to: text,
            line: index + 1,
        }))
    }

    /// Add an unticked playlist entry, so a later confirmation has a row to
    /// tick.
    ///
    /// `mark_playlist` deliberately refuses to invent a line, which is right
    /// when the operator names a playlist by hand. But a topic playlist is
    /// only ever known at the point the API call is made, so without this the
    /// row never exists and the entry can never be recorded at all (F5).
    ///
    /// Unticked is the whole point: the row says this playlist was asked for,
    /// not that the destination confirmed it. Only `mark_playlist` ticks.
    pub fn add_playlist(&mut self, playlist: &str, label: &str) -> Result<Option<Change>> {
        let info = self.structure();
        let (_, body) = self.locate_section(PLAYLIST_HEADINGS, &info)?;

        let checkboxes = body
            .clone()
            .filter(|index| !info[*index].fenced && checkbox(&self.lines[*index].text).is_some())
            .collect::<Vec<_>>();
        if checkboxes
            .iter()
            .any(|index| self.lines[*index].text.contains(playlist))
        {
            return Ok(None);
        }

        let text = format!("- [ ] {label} ({playlist})");
        // The skeleton leaves a `[TBD]` checkbox for exactly this, so fill it
        // rather than leaving a placeholder next to a real entry.
        let placeholder = checkboxes.iter().copied().find(|index| {
            let line = self.lines[*index].text.trim_end();
            line.ends_with("[TBD]") || line.ends_with(PLACEHOLDER)
        });

        let (line, from) = match placeholder {
            Some(index) => {
                let from = self.lines[index].text.clone();
                self.lines[index].text = text.clone();
                (index + 1, from)
            }
            None => {
                let at = checkboxes.last().map_or(body.end, |index| index + 1);
                let terminator = self
                    .lines
                    .get(at.saturating_sub(1))
                    .filter(|line| !line.terminator.is_empty())
                    .map_or("\n".to_owned(), |line| line.terminator.clone());
                self.lines.insert(
                    at,
                    Line {
                        text: text.clone(),
                        terminator,
                    },
                );
                (at + 1, String::new())
            }
        };

        Ok(Some(Change {
            section: Some(PLAYLIST_HEADINGS[0].to_owned()),
            label: playlist.to_owned(),
            from,
            to: text,
            line,
        }))
    }

    /// Replace the generated tag list.
    ///
    /// Most documents keep tags in a fenced block, a few keep them as bare
    /// prose, and one carries a derived character count. All three shapes are
    /// updated in place rather than normalised to one, because normalising
    /// would rewrite lines this command was not asked to touch.
    pub fn set_tags(&mut self, tags: &str) -> Result<Vec<Change>> {
        let info = self.structure();
        let (heading, body) = self.locate_section(&["Tags Used"], &info)?;
        let tags = tags.trim();

        let mut changes = Vec::new();
        let replaced = if body.clone().any(|index| info[index].fenced) {
            self.replace_fence(heading, tags)?
        } else {
            self.replace_prose(&body, tags, heading)
        };
        if let Some(change) = replaced {
            changes.push(Change {
                section: Some("Tags Used".to_owned()),
                label: "tags".to_owned(),
                ..change
            });
        }

        // The count is derived, so leaving it stale would be worse than not
        // having it at all. It is updated only where it already exists.
        let info = self.structure();
        let (_, body) = self.locate_section(&["Tags Used"], &info)?;
        if let Some(index) = body
            .clone()
            .find(|index| self.lines[*index].text.starts_with(TAG_COUNT_LABEL))
        {
            let from = self.lines[index].text.clone();
            let tick = if tags.chars().count() <= TAG_LIMIT {
                " ✓"
            } else {
                " ✗"
            };
            let to = format!(
                "{TAG_COUNT_LABEL} {}/{TAG_LIMIT}{tick}",
                tags.chars().count()
            );
            if from != to {
                self.lines[index].text = to.clone();
                changes.push(Change {
                    section: Some("Tags Used".to_owned()),
                    label: "Tag character count".to_owned(),
                    from,
                    to,
                    line: index + 1,
                });
            }
        }
        Ok(changes)
    }

    /// Add a missing social platform section.
    ///
    /// The documented insertion point is immediately before the manual
    /// YouTube Studio steps; documents without that heading get the section
    /// appended to the end of the publicity section instead. Existing sections
    /// are never rewritten.
    pub fn add_platform(&mut self, platform: &str) -> Result<Option<Change>> {
        let info = self.structure();
        if info.iter().enumerate().any(|(index, line)| {
            line.heading.is_some()
                && parse_heading(&self.lines[index].text)
                    .is_some_and(|(_, title)| title.eq_ignore_ascii_case(platform))
        }) {
            return Ok(None);
        }

        let (_, publicity) = self.locate_section(&["Social Media & Publicity"], &info)?;
        let at = info
            .iter()
            .enumerate()
            .find(|(index, line)| {
                line.heading.is_some()
                    && parse_heading(&self.lines[*index].text)
                        .is_some_and(|(_, title)| title.starts_with(MANUAL_STEPS_HEADING))
            })
            .map(|(index, _)| index)
            .unwrap_or(publicity.end);

        let terminator = self
            .lines
            .get(at.saturating_sub(1))
            .filter(|line| !line.terminator.is_empty())
            .map_or("\n".to_owned(), |line| line.terminator.clone());
        let block = [
            format!("### {platform}"),
            String::new(),
            "- **Post URL**: [To be added]".to_owned(),
            "- **Post Date**: [To be added]".to_owned(),
            String::new(),
            "#### Draft".to_owned(),
            String::new(),
            "```text".to_owned(),
            "[To be added]".to_owned(),
            "```".to_owned(),
            String::new(),
        ];
        let to = block.join("\n");
        self.lines.splice(
            at..at,
            block.into_iter().map(|text| Line {
                text,
                terminator: terminator.clone(),
            }),
        );

        Ok(Some(Change {
            section: Some("Social Media & Publicity".to_owned()),
            label: format!("{platform} section"),
            from: String::new(),
            to,
            line: at + 1,
        }))
    }

    /// Replace the contents of the first fenced block after `start`.
    fn replace_fence(&mut self, start: usize, text: &str) -> Result<Option<Change>> {
        let (open, close) = self.locate_fence(start)?;
        let current = self.lines[open + 1..close]
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if current == text {
            return Ok(None);
        }

        let terminator = self
            .lines
            .get(open + 1)
            .filter(|line| !line.terminator.is_empty())
            .map_or_else(
                || self.lines[open].terminator.clone(),
                |line| line.terminator.clone(),
            );
        let replacement = text
            .split('\n')
            .map(|text| Line {
                text: text.to_owned(),
                terminator: terminator.clone(),
            })
            .collect::<Vec<_>>();
        self.lines.splice(open + 1..close, replacement);

        Ok(Some(Change {
            section: None,
            label: "block".to_owned(),
            from: current,
            to: text.to_owned(),
            line: open + 2,
        }))
    }

    /// Replace the prose lines of a section, ignoring blank lines and any
    /// derived metadata line that follows them.
    fn replace_prose(
        &mut self,
        body: &std::ops::Range<usize>,
        text: &str,
        heading: usize,
    ) -> Option<Change> {
        let content = body
            .clone()
            .filter(|index| {
                let line = self.lines[*index].text.trim();
                !line.is_empty() && !line.starts_with(TAG_COUNT_LABEL)
            })
            .collect::<Vec<_>>();
        let current = content
            .iter()
            .map(|index| self.lines[*index].text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if current == text {
            return None;
        }

        let terminator = self.lines[heading].terminator.clone();
        let line = Line {
            text: text.to_owned(),
            terminator,
        };
        let at = match content.as_slice() {
            [] => heading + 1,
            [first, ..] => *first,
        };
        let end = content.last().map_or(at, |last| last + 1);
        self.lines.splice(at..end, [line]);

        Some(Change {
            section: None,
            label: "block".to_owned(),
            from: current,
            to: text.to_owned(),
            line: at + 1,
        })
    }
}

/// Both spellings occur in real documents: the planner writes the first, the
/// publication tooling has written the second.
const PLAYLIST_HEADINGS: &[&str] = &["Playlists to Add", "Playlists Added To", "Playlists"];
/// The skeleton's unfilled-value marker, recognised so a generated
/// placeholder is filled rather than left beside a real entry.
const PLACEHOLDER: &str = "[To be added]";
const MANUAL_STEPS_HEADING: &str = "Manual YouTube Studio Steps";
const TAG_COUNT_LABEL: &str = "**Tag character count**:";
const TAG_LIMIT: usize = 500;

/// Classification of one line, computed in a single pass.
struct LineInfo {
    fenced: bool,
    /// Heading level, when this line opens a heading.
    heading: Option<usize>,
    headings: Vec<String>,
}

/// Whether a checkbox list item is ticked, and the byte offset of the marker
/// character between the brackets.
fn checkbox(text: &str) -> Option<(bool, usize)> {
    let indent = text.len() - text.trim_start().len();
    let rest = &text[indent..];
    let marker = rest.strip_prefix("- [")?.chars().next()?;
    match marker {
        'x' | 'X' => Some((true, indent + 3)),
        ' ' => Some((false, indent + 3)),
        _ => None,
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

impl Document {
    /// Every managed field outside a fenced block, as
    /// `(enclosing headings, label, value)`.
    ///
    /// Readers use the same scan as writers so the two cannot disagree about
    /// what counts as a field.
    pub fn fields(&self) -> Vec<(Vec<String>, String, String)> {
        self.scan()
            .filter_map(|(_, headings, line)| {
                let label = field_label(&line.text)?;
                let (_, value) = field_value(&line.text)?;
                Some((headings, label.to_owned(), value.to_owned()))
            })
            .collect()
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
        Document::parse(Path::new("fixture.episode-data.md"), source)
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

    /// The shape real documents use, which differs from the written contract:
    /// two spellings of the playlist heading, checkboxes that also appear in
    /// the manual steps, and tags in a fence.
    const PUBLICATION: &str = "\
## YouTube

### Playlists to Add
- [x] Binge Watch (PLbinge) - Added 2026-03-16T18:35:13Z
- [ ] Clinical Safety (PLsafety)
- [ ] Audio-Only Podcast Version - add manually in YouTube Studio

### Tags Used

```
[Generated by tools/edh-tags]
```

**Tag character count**: 25/500 ✓

## Social Media & Publicity

### Bluesky
- **Post URL**: [To be added]

## Manual YouTube Studio Steps (after upload)

- [ ] Verify license is \"Creative Commons - Attribution\"
- [ ] Upload thumbnail when ready

## Notes

Human notes.
";

    #[test]
    fn ticks_only_the_named_playlist() {
        let mut doc = document(PUBLICATION);
        let change = doc
            .mark_playlist("PLsafety", Some("Added 2026-08-07T09:00:00Z"))
            .unwrap()
            .unwrap();
        assert_eq!(
            change.to,
            "- [x] Clinical Safety (PLsafety) - Added 2026-08-07T09:00:00Z"
        );

        let rendered = doc.render();
        assert!(rendered.contains("- [ ] Verify license"));
        assert!(rendered.contains("- [ ] Upload thumbnail when ready"));
        assert!(rendered.contains("- [ ] Audio-Only Podcast Version"));
        assert!(rendered.contains("- [x] Binge Watch (PLbinge) - Added 2026-03-16T18:35:13Z"));
    }

    #[test]
    fn ticking_an_added_playlist_again_changes_nothing() {
        let mut doc = document(PUBLICATION);
        assert!(
            doc.mark_playlist("PLbinge", Some("ignored"))
                .unwrap()
                .is_none()
        );
        assert_eq!(doc.render(), PUBLICATION);
    }

    #[test]
    fn reports_an_unlisted_playlist_rather_than_inventing_an_entry() {
        let mut doc = document(PUBLICATION);
        let error = doc
            .mark_playlist("PLunknown", None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("PLunknown"), "unexpected error: {error}");
        assert_eq!(doc.render(), PUBLICATION);
    }

    #[test]
    fn accepts_the_other_playlist_heading_spelling() {
        let source = PUBLICATION.replace("### Playlists to Add", "### Playlists Added To");
        let mut doc = document(&source);
        assert!(doc.mark_playlist("PLsafety", None).unwrap().is_some());
    }

    #[test]
    fn adds_an_unticked_row_for_a_playlist_the_record_does_not_list() {
        let mut doc = document(PUBLICATION);
        let change = doc
            .add_playlist("PLtopic", "Topic playlist")
            .unwrap()
            .unwrap();
        assert_eq!(change.to, "- [ ] Topic playlist (PLtopic)");
        assert_eq!(change.from, "", "an added row replaces nothing");

        let rendered = doc.render();
        assert!(
            rendered.contains(
                "- [ ] Audio-Only Podcast Version - add manually in YouTube Studio\n- [ ] Topic playlist (PLtopic)\n"
            ),
            "the row belongs with the others, not at the end of the section: {rendered}"
        );
        // Unticked is the point: the row says it was asked for, not confirmed.
        assert!(!rendered.contains("- [x] Topic playlist"));
        assert!(rendered.contains("- [x] Binge Watch (PLbinge) - Added 2026-03-16T18:35:13Z"));
        assert!(rendered.contains("Human notes."));
    }

    #[test]
    fn a_generated_placeholder_is_filled_rather_than_left_beside_a_real_entry() {
        // The shape `episode new` writes: a `[TBD]` checkbox awaiting the
        // topic playlist that is only known when the upload runs.
        let source = PUBLICATION.replace(
            "- [ ] Audio-Only Podcast Version - add manually in YouTube Studio\n",
            "- [ ] Audio-Only Podcast Version - add manually in YouTube Studio\n\nTopic-specific (relevant to this episode):\n\n- [ ] [TBD]\n",
        );
        let mut doc = document(&source);
        let change = doc
            .add_playlist("PLtopic", "Topic playlist")
            .unwrap()
            .unwrap();
        assert_eq!(change.from, "- [ ] [TBD]");

        let rendered = doc.render();
        assert!(rendered.contains("- [ ] Topic playlist (PLtopic)"));
        assert!(!rendered.contains("[TBD]"), "the placeholder must be used");
        assert_eq!(
            rendered.matches("Topic playlist").count(),
            1,
            "filling must not also append"
        );
    }

    #[test]
    fn adding_a_playlist_already_listed_changes_nothing() {
        let mut doc = document(PUBLICATION);
        // Ticked and unticked existing entries are both already present.
        for playlist in ["PLbinge", "PLsafety"] {
            assert!(
                doc.add_playlist(playlist, "Topic playlist")
                    .unwrap()
                    .is_none(),
                "{playlist} is already listed"
            );
        }
        assert_eq!(doc.render(), PUBLICATION);
    }

    #[test]
    fn replaces_fenced_tags_and_refreshes_the_derived_count() {
        let mut doc = document(PUBLICATION);
        let changes = doc.set_tags("nhs, digital health, informatics").unwrap();
        assert_eq!(changes.len(), 2);

        let rendered = doc.render();
        assert!(rendered.contains("```\nnhs, digital health, informatics\n```"));
        assert!(rendered.contains("**Tag character count**: 32/500 ✓"));
        assert!(rendered.contains("Human notes."));
    }

    #[test]
    fn marks_an_over_long_tag_list() {
        let mut doc = document(PUBLICATION);
        doc.set_tags(&"a,".repeat(300)).unwrap();
        assert!(doc.render().contains("/500 ✗"));
    }

    #[test]
    fn replaces_unfenced_tags_in_place() {
        let source = "### Tags Used\n[To be added]\n\n## Guest Review\n\n- **Changed to public**: [To be added]\n";
        let mut doc = document(source);
        doc.set_tags("nhs, digital health").unwrap();
        assert_eq!(
            doc.render(),
            "### Tags Used\nnhs, digital health\n\n## Guest Review\n\n- **Changed to public**: [To be added]\n"
        );
    }

    #[test]
    fn adds_a_missing_platform_before_the_manual_steps() {
        let mut doc = document(PUBLICATION);
        assert!(doc.add_platform("Mastodon").unwrap().is_some());

        let rendered = doc.render();
        let mastodon = rendered.find("### Mastodon").unwrap();
        assert!(mastodon > rendered.find("### Bluesky").unwrap());
        assert!(mastodon < rendered.find("## Manual YouTube Studio Steps").unwrap());
        assert!(rendered.contains("- **Post URL**: [To be added]"));

        // The new section is usable by the other commands straight away.
        doc.set_draft("Mastodon", "Draft", "Hello.").unwrap();
        assert!(doc.render().contains("```text\nHello.\n```"));
    }

    #[test]
    fn adding_an_existing_platform_changes_nothing() {
        let mut doc = document(PUBLICATION);
        assert!(doc.add_platform("Bluesky").unwrap().is_none());
        assert_eq!(doc.render(), PUBLICATION);
    }

    #[test]
    fn adds_a_platform_to_a_document_without_manual_steps() {
        let source = "## Social Media & Publicity\n\n### Bluesky\n- **Post URL**: [To be added]\n\n## Notes\n\nHuman notes.\n";
        let mut doc = document(source);
        assert!(doc.add_platform("LinkedIn").unwrap().is_some());
        let rendered = doc.render();
        assert!(rendered.find("### LinkedIn").unwrap() < rendered.find("## Notes").unwrap());
        assert!(rendered.ends_with("Human notes.\n"));
    }

    #[test]
    fn exposes_fields_with_their_sections() {
        let fields = document(CANONICAL).fields();
        let post_url = fields
            .iter()
            .find(|(_, label, _)| label == "Post URL")
            .unwrap();
        assert_eq!(post_url.0.last().unwrap(), "Bluesky");
        assert!(
            !fields
                .iter()
                .any(|(_, _, value)| value.contains("Original human-reviewed"))
        );
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
