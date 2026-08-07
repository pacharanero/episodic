// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Deterministic media work: audio extraction, loudness processing, and
//! transcription.
//!
//! Every output is staged beside its destination, validated, and only then
//! renamed into place. A tool that fails, or that exits successfully having
//! written something unreadable, must never leave a plausible-looking artefact
//! that later steps will treat as finished work.

use crate::channel::AudioPolicy;
use crate::tool::Tool;
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Artefacts Whisper must produce before any of them are published.
const TRANSCRIPT_EXTENSIONS: &[&str] = &["srt", "vtt", "txt", "json", "tsv"];

#[derive(Debug, Serialize)]
pub struct MediaOutcome {
    pub input: PathBuf,
    pub outputs: Vec<PathBuf>,
    /// What would be run, so a dry run shows the real command.
    pub commands: Vec<String>,
    pub applied: bool,
    /// True when the work was already done and nothing needed doing.
    pub skipped: bool,
    pub detail: Option<String>,
}

impl MediaOutcome {
    pub fn as_text(&self) -> String {
        let mut lines = Vec::new();
        if self.skipped {
            lines.push(format!(
                "Already done: {}",
                self.outputs
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            return lines.join("\n");
        }
        lines.push(format!(
            "{} {}",
            if self.applied { "Wrote" } else { "Would write" },
            self.outputs
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        if let Some(detail) = &self.detail {
            lines.push(format!("  {detail}"));
        }
        if !self.applied {
            lines.extend(self.commands.iter().map(|command| format!("  {command}")));
        }
        lines.join("\n")
    }
}

/// Loudness measurements reported by `ffmpeg`'s loudnorm filter.
#[derive(Debug, Serialize)]
pub struct Loudness {
    pub input: PathBuf,
    pub integrated_lufs: f64,
    pub true_peak_dbtp: f64,
    pub loudness_range_lu: f64,
    pub threshold_lufs: f64,
    /// Whether integrated loudness is at the channel target.
    pub loudness_on_target: bool,
    /// Whether the true peak is within the channel ceiling.
    pub peak_within_ceiling: bool,
    pub targets: Targets,
    pub advice: String,
}

#[derive(Debug, Serialize)]
pub struct Targets {
    pub integrated_lufs: f64,
    pub true_peak_dbtp: f64,
    pub loudness_range_lu: f64,
}

impl Loudness {
    /// What the numbers mean for this file.
    ///
    /// The two measurements fail for different reasons and need different
    /// answers. Loudness off target means the chain has not been applied.
    /// A true peak slightly over the ceiling on otherwise on-target audio is
    /// normally the lossy encoder overshooting after loudnorm measured the
    /// signal, not a reason to process the file again: re-processing already
    /// normalised audio compresses it a second time.
    fn advise(loudness_on_target: bool, peak_within_ceiling: bool) -> String {
        match (loudness_on_target, peak_within_ceiling) {
            (true, true) => "Within target; nothing to do.".to_owned(),
            (true, false) => {
                "Loudness is on target but the true peak is over the ceiling. This is usually the encoder overshooting after measurement. Do not re-process; lower audio.true_peak if it matters."
                    .to_owned()
            }
            (false, _) => "Loudness is off target; run `eps audio process`.".to_owned(),
        }
    }

    pub fn as_text(&self) -> String {
        let mark = |ok: bool| if ok { "ok" } else { "over" };
        format!(
            "Loudness of {}\n  Integrated:     {:.1} LUFS (target {:.1}) {}\n  True peak:      {:.1} dBTP (ceiling {:.1}) {}\n  Loudness range: {:.1} LU (target {:.1})\n  Threshold:      {:.1} LUFS\n  {}",
            self.input.display(),
            self.integrated_lufs,
            self.targets.integrated_lufs,
            if self.loudness_on_target { "ok" } else { "off" },
            self.true_peak_dbtp,
            self.targets.true_peak_dbtp,
            mark(self.peak_within_ceiling),
            self.loudness_range_lu,
            self.targets.loudness_range_lu,
            self.threshold_lufs,
            self.advice
        )
    }
}

pub struct Media {
    ffmpeg: Tool,
    ffprobe: Tool,
}

impl Media {
    pub fn new() -> Result<Self> {
        Ok(Self {
            ffmpeg: Tool::find("ffmpeg", &[])?,
            ffprobe: Tool::find("ffprobe", &[])?,
        })
    }

    /// Whether a file exists and can be parsed as media.
    ///
    /// Used both to validate what a tool produced and to decide whether work
    /// has already been done. A zero-length or truncated file is not "done".
    pub fn is_readable(&self, path: &Path) -> bool {
        path.metadata().is_ok_and(|metadata| metadata.len() > 0)
            && self
                .ffprobe
                .run(&["-v", "error", &path.display().to_string()])
                .is_ok()
    }

    fn has_video_stream(&self, path: &Path) -> Result<bool> {
        let output = self.ffprobe.run(&[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "default=nw=1:nk=1",
            &path.display().to_string(),
        ])?;
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line.trim() == "video"))
    }

    /// Extract the standardised mono track used for transcription.
    pub fn extract_audio(
        &self,
        input: &Path,
        output: Option<&Path>,
        policy: &AudioPolicy,
        dry_run: bool,
    ) -> Result<MediaOutcome> {
        let output = match output {
            Some(path) => path.to_path_buf(),
            None => with_extension(input, "wav"),
        };
        let rate = policy.transcription_sample_rate.to_string();
        let arguments = vec![
            "-hide_banner".to_owned(),
            "-loglevel".to_owned(),
            "error".to_owned(),
            "-y".to_owned(),
            "-i".to_owned(),
            input.display().to_string(),
            "-vn".to_owned(),
            "-ac".to_owned(),
            "1".to_owned(),
            "-ar".to_owned(),
            rate,
            "-c:a".to_owned(),
            "pcm_s16le".to_owned(),
        ];

        self.render(input, &output, arguments, dry_run, None)
    }

    /// Apply the channel's loudness chain.
    ///
    /// Two passes: the first measures the material after preprocessing, the
    /// second applies linear normalisation using those measurements. A single
    /// pass would have to guess, and would pump quiet passages.
    pub fn process_audio(
        &self,
        input: &Path,
        output: Option<&Path>,
        policy: &AudioPolicy,
        dry_run: bool,
    ) -> Result<MediaOutcome> {
        let output = match output {
            Some(path) => path.to_path_buf(),
            None => processed_path(input),
        };
        if self.is_readable(&output) {
            return Ok(skipped(input, output));
        }

        let preprocess = policy.preprocess_chain();
        let measure = format!(
            "{preprocess},{}:print_format=json",
            policy.loudnorm_targets()
        );
        let measure_arguments = vec![
            "-hide_banner".to_owned(),
            "-nostats".to_owned(),
            "-vn".to_owned(),
            "-i".to_owned(),
            input.display().to_string(),
            "-af".to_owned(),
            measure,
            "-f".to_owned(),
            "null".to_owned(),
            "-".to_owned(),
        ];

        if dry_run {
            return Ok(MediaOutcome {
                input: input.to_path_buf(),
                outputs: vec![output],
                commands: vec![
                    self.ffmpeg.command_line(&measure_arguments),
                    format!(
                        "{} ... -af '{preprocess},{}:measured_*=<from pass 1>:linear=true' <output>",
                        self.ffmpeg.path.display(),
                        policy.loudnorm_targets()
                    ),
                ],
                applied: false,
                skipped: false,
                detail: Some("two-pass loudness normalisation".to_owned()),
            });
        }

        let measured = self.measure(input, policy, &measure_arguments)?;
        let filter = format!(
            "{preprocess},{}:measured_I={}:measured_TP={}:measured_LRA={}:measured_thresh={}:linear=true:print_format=summary",
            policy.loudnorm_targets(),
            measured.integrated_lufs,
            measured.true_peak_dbtp,
            measured.loudness_range_lu,
            measured.threshold_lufs
        );

        let rate = policy.output_sample_rate.to_string();
        let bitrate = format!("{}k", policy.aac_bitrate_kbps);
        let mut arguments = vec![
            "-hide_banner".to_owned(),
            "-loglevel".to_owned(),
            "warning".to_owned(),
            "-y".to_owned(),
            "-i".to_owned(),
            input.display().to_string(),
        ];
        if self.has_video_stream(input)? {
            // The picture is untouched: only the audio track is replaced, so
            // processing never costs a video re-encode or a generation of
            // quality loss.
            arguments.extend(
                [
                    "-map",
                    "0:v:0",
                    "-c:v",
                    "copy",
                    "-map",
                    "0:a:0",
                    "-af",
                    &filter,
                    "-c:a",
                    "aac",
                    "-b:a",
                    &bitrate,
                    "-ar",
                    &rate,
                    "-movflags",
                    "+faststart",
                ]
                .map(str::to_owned),
            );
        } else {
            let codec = match extension_of(&output).as_str() {
                "wav" | "aiff" | "aif" => vec!["-c:a", "pcm_s16le"],
                "flac" => vec!["-c:a", "flac"],
                _ => vec!["-c:a", "aac", "-b:a", &bitrate],
            };
            arguments.extend(["-af", &filter].map(str::to_owned));
            arguments.extend(codec.into_iter().map(str::to_owned));
            arguments.extend(["-ar", &rate].map(str::to_owned));
        }

        self.render(
            input,
            &output,
            arguments,
            false,
            Some(format!(
                "measured {:.1} LUFS, normalised to {:.1} LUFS",
                measured.integrated_lufs, policy.target_lufs
            )),
        )
    }

    /// Measure loudness without changing anything.
    pub fn analyse_audio(&self, input: &Path, policy: &AudioPolicy) -> Result<Loudness> {
        if !self.is_readable(input) {
            bail!("{} is not readable media", input.display());
        }
        let arguments = vec![
            "-hide_banner".to_owned(),
            "-nostats".to_owned(),
            "-vn".to_owned(),
            "-i".to_owned(),
            input.display().to_string(),
            "-af".to_owned(),
            format!("{}:print_format=json", policy.loudnorm_targets()),
            "-f".to_owned(),
            "null".to_owned(),
            "-".to_owned(),
        ];
        self.measure(input, policy, &arguments)
    }

    fn measure(
        &self,
        input: &Path,
        policy: &AudioPolicy,
        arguments: &[String],
    ) -> Result<Loudness> {
        let output = self.ffmpeg.run(arguments)?;
        let log = String::from_utf8_lossy(&output.stderr);
        let json = last_json_object(&log).with_context(|| {
            format!(
                "could not find loudness measurements in ffmpeg output for {}",
                input.display()
            )
        })?;
        let measured: serde_json::Value = serde_json::from_str(&json)
            .context("ffmpeg reported loudness measurements that are not valid JSON")?;

        let read = |key: &str| -> Result<f64> {
            let raw = measured
                .get(key)
                .and_then(|value| value.as_str())
                .with_context(|| format!("ffmpeg did not report {key}"))?;
            raw.parse::<f64>()
                .with_context(|| format!("ffmpeg reported {key} as '{raw}', which is not a number"))
        };

        let integrated_lufs = read("input_i")?;
        let true_peak_dbtp = read("input_tp")?;
        let loudness_range_lu = read("input_lra")?;
        // A LUFS measurement is not repeatable to better than a fraction of a
        // dB, so an exact comparison would report noise as a failure.
        let loudness_on_target = (integrated_lufs - policy.target_lufs).abs() <= 0.5;
        let peak_within_ceiling = true_peak_dbtp <= policy.true_peak;

        Ok(Loudness {
            input: input.to_path_buf(),
            integrated_lufs,
            true_peak_dbtp,
            loudness_range_lu,
            threshold_lufs: read("input_thresh")?,
            loudness_on_target,
            peak_within_ceiling,
            advice: Loudness::advise(loudness_on_target, peak_within_ceiling),
            targets: Targets {
                integrated_lufs: policy.target_lufs,
                true_peak_dbtp: policy.true_peak,
                loudness_range_lu: policy.loudness_range,
            },
        })
    }

    /// Run ffmpeg into a staged file, validate it, then move it into place.
    fn render(
        &self,
        input: &Path,
        output: &Path,
        mut arguments: Vec<String>,
        dry_run: bool,
        detail: Option<String>,
    ) -> Result<MediaOutcome> {
        if self.is_readable(output) {
            return Ok(skipped(input, output.to_path_buf()));
        }
        if !input.is_file() {
            bail!("input not found: {}", input.display());
        }

        let directory = parent_of(output);
        let staged = directory.join(format!(
            ".{}.staging.{}",
            file_name(output),
            extension_of(output)
        ));
        arguments.push(staged.display().to_string());

        if dry_run {
            let mut display = arguments.clone();
            display.pop();
            display.push(output.display().to_string());
            return Ok(MediaOutcome {
                input: input.to_path_buf(),
                outputs: vec![output.to_path_buf()],
                commands: vec![self.ffmpeg.command_line(&display)],
                applied: false,
                skipped: false,
                detail,
            });
        }

        fs::create_dir_all(&directory)?;
        let _ = fs::remove_file(&staged);
        let result = self.ffmpeg.run(&arguments);
        if result.is_err() {
            let _ = fs::remove_file(&staged);
        }
        result?;

        // ffmpeg can exit zero having written something unusable, so the
        // staged file is probed before it is allowed to become the artefact.
        if !self.is_readable(&staged) {
            let _ = fs::remove_file(&staged);
            bail!(
                "{} exited successfully but produced unreadable output for {}",
                self.ffmpeg.name,
                output.display()
            );
        }
        fs::rename(&staged, output)
            .with_context(|| format!("could not move staged output into {}", output.display()))?;

        Ok(MediaOutcome {
            input: input.to_path_buf(),
            outputs: vec![output.to_path_buf()],
            commands: Vec::new(),
            applied: true,
            skipped: false,
            detail,
        })
    }
}

/// Transcribe audio, publishing the transcript family only once every member
/// of it exists. A partial transcript is worse than none: later steps read the
/// SRT and would silently work from a truncated one.
pub fn transcribe(
    input: &Path,
    output_dir: Option<&Path>,
    model: &str,
    language: &str,
    device: &str,
    workspace_root: &Path,
    dry_run: bool,
) -> Result<MediaOutcome> {
    if !input.is_file() {
        bail!("input not found: {}", input.display());
    }
    let whisper = Tool::find("whisper", &[workspace_root.join("venv/bin/whisper")])?;
    let directory = output_dir.map_or_else(|| parent_of(input), Path::to_path_buf);
    let base = stem_of(input);
    let outputs = TRANSCRIPT_EXTENSIONS
        .iter()
        .map(|extension| directory.join(format!("{base}.{extension}")))
        .collect::<Vec<_>>();

    let srt = directory.join(format!("{base}.srt"));
    if srt.metadata().is_ok_and(|metadata| metadata.len() > 0) {
        return Ok(skipped(input, srt));
    }

    fs::create_dir_all(&directory)?;
    let staging = tempfile::Builder::new()
        .prefix(&format!(".{base}.transcribe."))
        .tempdir_in(&directory)?;

    let arguments = vec![
        input.display().to_string(),
        "--model".to_owned(),
        model.to_owned(),
        "--language".to_owned(),
        language.to_owned(),
        "--device".to_owned(),
        device.to_owned(),
        "--output_format".to_owned(),
        "all".to_owned(),
        "--output_dir".to_owned(),
        staging.path().display().to_string(),
        "--fp16".to_owned(),
        "False".to_owned(),
        "--verbose".to_owned(),
        "False".to_owned(),
    ];

    if dry_run {
        return Ok(MediaOutcome {
            input: input.to_path_buf(),
            outputs,
            commands: vec![whisper.command_line(&arguments)],
            applied: false,
            skipped: false,
            detail: Some(format!(
                "model {model}, language {language}, device {device}"
            )),
        });
    }

    eprintln!("Transcribing with {model} on {device}; this takes a while.");
    whisper.run_attached(&arguments)?;

    for extension in TRANSCRIPT_EXTENSIONS {
        let produced = staging.path().join(format!("{base}.{extension}"));
        if !produced.metadata().is_ok_and(|metadata| metadata.len() > 0) {
            bail!("whisper did not produce a non-empty {extension} transcript");
        }
    }
    for extension in TRANSCRIPT_EXTENSIONS {
        fs::rename(
            staging.path().join(format!("{base}.{extension}")),
            directory.join(format!("{base}.{extension}")),
        )?;
    }

    Ok(MediaOutcome {
        input: input.to_path_buf(),
        outputs,
        commands: Vec::new(),
        applied: true,
        skipped: false,
        detail: Some(format!(
            "model {model}, language {language}, device {device}"
        )),
    })
}

fn skipped(input: &Path, output: PathBuf) -> MediaOutcome {
    MediaOutcome {
        input: input.to_path_buf(),
        outputs: vec![output],
        commands: Vec::new(),
        applied: false,
        skipped: true,
        detail: None,
    }
}

/// The last JSON object in a log, which is where loudnorm prints its report.
fn last_json_object(log: &str) -> Option<String> {
    let start = log.rfind('{')?;
    let end = log[start..].find('}')? + start;
    Some(log[start..=end].to_owned())
}

fn parent_of(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn stem_of(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn with_extension(input: &Path, extension: &str) -> PathBuf {
    parent_of(input).join(format!("{}.{extension}", stem_of(input)))
}

/// `episode.mp4` becomes `episode.processed.mp4`, matching the artefact
/// contract's name for the upload master.
fn processed_path(input: &Path) -> PathBuf {
    let extension = extension_of(input);
    parent_of(input).join(if extension.is_empty() {
        format!("{}.processed", stem_of(input))
    } else {
        format!("{}.processed.{extension}", stem_of(input))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_upload_master_by_contract() {
        assert_eq!(
            processed_path(Path::new("/e/2026.07.29-x.mp4")),
            PathBuf::from("/e/2026.07.29-x.processed.mp4")
        );
        assert_eq!(
            processed_path(Path::new("clip.MKV")),
            PathBuf::from("./clip.processed.mkv")
        );
    }

    #[test]
    fn names_the_transcription_track_beside_its_source() {
        assert_eq!(
            with_extension(Path::new("/e/show.mp4"), "wav"),
            PathBuf::from("/e/show.wav")
        );
    }

    #[test]
    fn finds_the_loudness_report_among_other_output() {
        let log = "frame= 100 fps=0\n[Parsed_loudnorm_3 @ 0x5] \n{\n\t\"input_i\" : \"-19.2\",\n\t\"input_tp\" : \"-3.1\"\n}\n";
        let json = last_json_object(log).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["input_i"], "-19.2");
    }

    #[test]
    fn distinguishes_unprocessed_audio_from_encoder_overshoot() {
        // Loudness off target: the chain has not been applied.
        assert!(Loudness::advise(false, true).contains("run `eps audio process`"));
        assert!(Loudness::advise(false, false).contains("run `eps audio process`"));
        // On-target loudness with a peak slightly over is the encoder, and
        // re-processing would compress already normalised audio a second time.
        let overshoot = Loudness::advise(true, false);
        assert!(overshoot.contains("Do not re-process"));
        assert!(!overshoot.contains("run `eps audio process`"));
        assert!(Loudness::advise(true, true).contains("nothing to do"));
    }

    #[test]
    fn reports_no_json_rather_than_guessing() {
        assert_eq!(last_json_object("no measurements here"), None);
    }
}
