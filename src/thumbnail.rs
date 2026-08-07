// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Frame extraction and thumbnail composition.
//!
//! Frames are candidates for a human to choose between, so extraction is
//! per-frame idempotent and never overwrites a frame that already exists.
//! Composition emits both the PNG that gets uploaded and an editable SVG, so
//! a thumbnail that is nearly right can be adjusted rather than regenerated.

use crate::channel::ThumbnailPolicy;
use crate::media::{Media, MediaOutcome};
use crate::tool::Tool;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

/// Fraction of the video skipped at each end, avoiding intro and outro slates.
const EDGE_MARGIN: f64 = 0.05;
/// Fraction of the canvas the caption may occupy. The vertical budget is
/// generous so that text wrapping to four or five lines still has room for the
/// stroke around the outermost glyphs.
const CAPTION_WIDTH: f64 = 0.86;
const CAPTION_HEIGHT: f64 = 0.89;

/// ImageMagick 7 renamed the entry point; `convert` remains as a deprecated
/// alias, so prefer `magick` and fall back for older installations.
fn imagemagick() -> Result<Tool> {
    Tool::find("magick", &[]).or_else(|_| Tool::find("convert", &[]))
}

/// Extract evenly spaced interior frames for thumbnail selection.
pub fn frames(video: &Path, count: u32, dry_run: bool) -> Result<MediaOutcome> {
    if count == 0 {
        bail!("frame count must be at least one");
    }
    let media = Media::new()?;
    if !video.is_file() {
        bail!("video not found: {}", video.display());
    }
    let duration = media.duration_seconds(video)?;
    if duration <= 0.0 {
        bail!("could not read a usable duration from {}", video.display());
    }

    let directory = frames_directory(video);
    let usable = duration * (1.0 - 2.0 * EDGE_MARGIN);
    let mut outputs = Vec::new();
    let mut commands = Vec::new();
    let mut written = 0;

    for index in 1..=count {
        let at = duration * EDGE_MARGIN + usable * f64::from(index) / f64::from(count + 1);
        let output = directory.join(format!("frame-{index:02}.jpg"));
        outputs.push(output.clone());

        // A frame that already exists may be the one already chosen, so it is
        // never regenerated.
        if output.metadata().is_ok_and(|meta| meta.len() > 0) {
            continue;
        }
        let arguments = frame_arguments(&at, video);
        if dry_run {
            commands.push(media.command_line(&arguments, &output));
            continue;
        }
        fs::create_dir_all(&directory)?;
        media.render_to(&arguments, &output)?;
        written += 1;
    }

    Ok(MediaOutcome {
        input: video.to_path_buf(),
        outputs,
        commands,
        applied: !dry_run && written > 0,
        skipped: !dry_run && written == 0,
        detail: Some(format!(
            "{written} new frame(s) of {count} in {}",
            directory.display()
        )),
    })
}

fn frame_arguments(at: &f64, video: &Path) -> Vec<String> {
    vec![
        "-hide_banner".to_owned(),
        "-loglevel".to_owned(),
        "error".to_owned(),
        "-y".to_owned(),
        // Seeking before the input is the fast path.
        "-ss".to_owned(),
        format!("{at:.3}"),
        "-i".to_owned(),
        video.display().to_string(),
        "-frames:v".to_owned(),
        "1".to_owned(),
        "-q:v".to_owned(),
        "2".to_owned(),
    ]
}

/// `<base>.frames`, beside the video.
///
/// `.processed` is stripped so frames taken from the upload master still land
/// in the episode's own frames directory.
fn frames_directory(video: &Path) -> PathBuf {
    let stem = video
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let base = stem.strip_suffix(".processed").unwrap_or(&stem);
    video
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
        .join(format!("{base}.frames"))
}

/// Overlay caption text on a frame, producing the upload PNG and an editable
/// SVG companion.
pub fn compose(
    image: &Path,
    text: &str,
    output: Option<&Path>,
    policy: &ThumbnailPolicy,
    workspace_root: &Path,
    quote: bool,
    dry_run: bool,
) -> Result<MediaOutcome> {
    if !image.is_file() {
        bail!("input image not found: {}", image.display());
    }
    if text.trim().is_empty() {
        bail!("thumbnail text must not be empty");
    }
    let magick = imagemagick()?;

    let font = policy
        .font_path
        .as_ref()
        .map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                workspace_root.join(path)
            }
        })
        .context("this channel has no thumbnail.font_path configured")?;
    if !font.is_file() {
        bail!("thumbnail font not found at {}", font.display());
    }

    let png = match output {
        Some(path) => path.to_path_buf(),
        None => default_output(image),
    };
    let svg = png.with_extension("svg");

    let quoted = if quote {
        format!("\"{text}\"")
    } else {
        text.to_owned()
    };
    let caption = if policy.uppercase.unwrap_or(true) {
        quoted.to_uppercase()
    } else {
        quoted
    };

    let canvas = format!("{}x{}", policy.width, policy.height);
    let caption_size = format!(
        "{}x{}",
        (f64::from(policy.width) * CAPTION_WIDTH) as u32,
        (f64::from(policy.height) * CAPTION_HEIGHT) as u32
    );

    if dry_run {
        return Ok(MediaOutcome {
            input: image.to_path_buf(),
            outputs: vec![png, svg],
            commands: vec![format!(
                "{} -background none -fill {} -stroke {} -font {} -size {caption_size} caption:{caption:?} ... -resize {canvas}^ -composite <output>",
                magick.path.display(),
                policy.fill.as_deref().unwrap_or("black"),
                policy.stroke.as_deref().unwrap_or("white"),
                font.display()
            )],
            applied: false,
            skipped: false,
            detail: Some(format!("caption: {caption}")),
        });
    }

    // Both files must exist to count as done: the PNG alone would hide a
    // half-finished pair that cannot be edited.
    if non_empty(&png) && non_empty(&svg) {
        return Ok(MediaOutcome {
            input: image.to_path_buf(),
            outputs: vec![png, svg],
            commands: Vec::new(),
            applied: false,
            skipped: true,
            detail: None,
        });
    }

    let directory = png
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    fs::create_dir_all(&directory)?;

    let caption_tile = tempfile::Builder::new()
        .prefix(".caption.")
        .suffix(".png")
        .tempfile_in(&directory)?;
    // `caption:` wraps and shrinks the text to fit the tile, so the configured
    // point size is a ceiling rather than a fixed size.
    magick.run(&[
        "-background".to_owned(),
        "none".to_owned(),
        "-fill".to_owned(),
        policy.fill.clone().unwrap_or_else(|| "black".to_owned()),
        "-stroke".to_owned(),
        policy.stroke.clone().unwrap_or_else(|| "white".to_owned()),
        "-strokewidth".to_owned(),
        policy.stroke_width.unwrap_or(4).to_string(),
        "-font".to_owned(),
        font.display().to_string(),
        "-pointsize".to_owned(),
        policy.point_size.unwrap_or(100).to_string(),
        "-gravity".to_owned(),
        "center".to_owned(),
        "-size".to_owned(),
        caption_size,
        format!("caption:{caption}"),
        caption_tile.path().display().to_string(),
    ])?;

    let staged_png = directory.join(format!(
        ".{}.staging.png",
        png.file_name().unwrap_or_default().to_string_lossy()
    ));
    let result = magick.run(&[
        image.display().to_string(),
        "-resize".to_owned(),
        format!("{canvas}^"),
        "-gravity".to_owned(),
        "center".to_owned(),
        "-extent".to_owned(),
        canvas.clone(),
        caption_tile.path().display().to_string(),
        "-gravity".to_owned(),
        "center".to_owned(),
        "-composite".to_owned(),
        staged_png.display().to_string(),
    ]);
    if result.is_err() {
        let _ = fs::remove_file(&staged_png);
    }
    result?;

    if !non_empty(&staged_png) {
        let _ = fs::remove_file(&staged_png);
        bail!(
            "ImageMagick produced an empty thumbnail for {}",
            png.display()
        );
    }

    let svg_source = build_svg(image, &svg, &caption, policy);
    fs::write(&svg, svg_source)?;
    fs::rename(&staged_png, &png)?;

    Ok(MediaOutcome {
        input: image.to_path_buf(),
        outputs: vec![png, svg],
        commands: Vec::new(),
        applied: true,
        skipped: false,
        detail: Some(format!("caption: {caption}")),
    })
}

fn non_empty(path: &Path) -> bool {
    path.metadata().is_ok_and(|meta| meta.len() > 0)
}

/// `<episode>/<base>.thumbnail.png`, climbing out of a `.frames` directory so
/// a chosen frame produces the episode's thumbnail rather than a nested one.
///
/// The path is made absolute first: climbing out of a relative
/// `<base>.frames/frame-03.jpg` otherwise lands on an empty parent, and the
/// episode name is what gives the thumbnail its name.
fn default_output(image: &Path) -> PathBuf {
    let absolute = std::path::absolute(image).unwrap_or_else(|_| image.to_path_buf());
    let mut directory = absolute
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    if directory
        .file_name()
        .is_some_and(|name| name.to_string_lossy().ends_with(".frames"))
        && let Some(parent) = directory.parent()
    {
        directory = parent.to_path_buf();
    }
    let base = directory
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "thumbnail".to_owned());
    directory.join(format!("{base}.thumbnail.png"))
}

/// An editable companion referencing the frame by relative path, with the
/// caption as separate lines so it can be adjusted in a vector editor.
fn build_svg(image: &Path, svg: &Path, caption: &str, policy: &ThumbnailPolicy) -> String {
    let width = policy.width;
    let height = policy.height;
    let point_size = policy.point_size.unwrap_or(100);
    let line_height = f64::from(point_size) * 1.1;
    let lines = wrap(caption, 18);
    let start = f64::from(height) / 2.0 - (lines.len() as f64 - 1.0) * line_height / 2.0;
    let centre = width / 2;

    let mut tspans = String::new();
    for (index, line) in lines.iter().enumerate() {
        let dy = if index == 0 { 0.0 } else { line_height };
        tspans.push_str(&format!(
            "      <tspan x=\"{centre}\" dy=\"{dy:.0}\">{}</tspan>\n",
            escape_xml(line)
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"
     width="{width}" height="{height}" viewBox="0 0 {width} {height}">
  <title>{title}</title>
  <image href="{href}" x="0" y="0" width="{width}" height="{height}"
         preserveAspectRatio="xMidYMid slice"/>
  <g font-size="{point_size}"
     fill="{fill}"
     stroke="{stroke}"
     stroke-width="{stroke_width}"
     stroke-linejoin="round"
     paint-order="stroke fill"
     text-anchor="middle">
    <text y="{start:.0}">
{tspans}    </text>
  </g>
</svg>
"#,
        title = escape_xml(caption),
        href = escape_xml(&relative_to(svg, image)),
        fill = policy.fill.as_deref().unwrap_or("black"),
        stroke = policy.stroke.as_deref().unwrap_or("white"),
        stroke_width = policy.stroke_width.unwrap_or(4),
    )
}

/// A path from the SVG's directory to the image, so the pair can be moved
/// together without breaking the reference.
fn relative_to(svg: &Path, image: &Path) -> String {
    let from = svg.parent().unwrap_or(Path::new("."));
    if let Ok(stripped) = image.strip_prefix(from) {
        return stripped.display().to_string();
    }
    if let (Some(image_parent), Some(name)) = (image.parent(), image.file_name())
        && image_parent.parent() == Some(from)
        && let Some(directory) = image_parent.file_name()
    {
        return format!("{}/{}", directory.to_string_lossy(), name.to_string_lossy());
    }
    image.display().to_string()
}

/// Wrap on word boundaries, roughly matching what the caption tile fits.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > width {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    // The real layout: the frames directory sits inside the episode folder.
    const EPISODE: &str = "/episodes/in-progress/2026.07.29-x";

    #[test]
    fn frames_land_beside_the_episode_not_inside_processed() {
        assert_eq!(
            frames_directory(Path::new(&format!("{EPISODE}/2026.07.29-x.processed.mp4"))),
            PathBuf::from(format!("{EPISODE}/2026.07.29-x.frames"))
        );
        assert_eq!(
            frames_directory(Path::new(&format!("{EPISODE}/2026.07.29-x.mp4"))),
            PathBuf::from(format!("{EPISODE}/2026.07.29-x.frames"))
        );
    }

    #[test]
    fn a_chosen_frame_produces_the_episode_thumbnail() {
        assert_eq!(
            default_output(Path::new(&format!(
                "{EPISODE}/2026.07.29-x.frames/frame-04.jpg"
            ))),
            PathBuf::from(format!("{EPISODE}/2026.07.29-x.thumbnail.png"))
        );
        // An image chosen from the episode folder itself still works.
        assert_eq!(
            default_output(Path::new(&format!("{EPISODE}/still.jpg"))),
            PathBuf::from(format!("{EPISODE}/2026.07.29-x.thumbnail.png"))
        );
    }

    #[test]
    fn a_relative_frame_path_still_names_the_thumbnail_after_the_episode() {
        // Running from inside the episode folder is the normal case, and a
        // relative path must not collapse to a nameless parent.
        let directory = tempfile::tempdir().unwrap();
        let episode = directory.path().join("2026.07.29-x");
        fs::create_dir_all(episode.join("2026.07.29-x.frames")).unwrap();
        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(&episode).unwrap();
        let output = default_output(Path::new("2026.07.29-x.frames/frame-03.jpg"));
        std::env::set_current_dir(previous).unwrap();
        assert_eq!(
            output.file_name().unwrap().to_string_lossy(),
            "2026.07.29-x.thumbnail.png"
        );
    }

    #[test]
    fn the_svg_references_the_frame_relatively() {
        assert_eq!(
            relative_to(
                Path::new("/e/ep/ep.thumbnail.svg"),
                Path::new("/e/ep/ep.frames/frame-04.jpg")
            ),
            "ep.frames/frame-04.jpg"
        );
    }

    #[test]
    fn wraps_on_word_boundaries() {
        assert_eq!(
            wrap("DESIGN EHRS TO HELP CLINICIANS THINK", 18),
            vec!["DESIGN EHRS TO", "HELP CLINICIANS", "THINK"]
        );
        assert_eq!(wrap("SHORT", 18), vec!["SHORT"]);
        // A single word longer than the line is kept whole rather than cut.
        assert_eq!(
            wrap("SUPERCALIFRAGILISTIC", 18),
            vec!["SUPERCALIFRAGILISTIC"]
        );
    }

    #[test]
    fn escapes_text_that_would_break_the_svg() {
        assert_eq!(
            escape_xml(r#""A & B" <tag>"#),
            "&quot;A &amp; B&quot; &lt;tag&gt;"
        );
    }

    #[test]
    fn builds_an_svg_carrying_every_caption_line() {
        let policy = ThumbnailPolicy {
            width: 1280,
            height: 720,
            ..ThumbnailPolicy::default()
        };
        let svg = build_svg(
            Path::new("/e/ep/ep.frames/frame-01.jpg"),
            Path::new("/e/ep/ep.thumbnail.svg"),
            "\"DESIGN EHRS TO HELP CLINICIANS THINK\"",
            &policy,
        );
        assert!(svg.contains("viewBox=\"0 0 1280 720\""));
        assert!(svg.contains("href=\"ep.frames/frame-01.jpg\""));
        assert_eq!(svg.matches("<tspan").count(), 3);
        assert!(!svg.contains("\"DESIGN"), "raw quotes must be escaped");
    }
}
