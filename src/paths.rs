// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

/// The `<base>` that every durable episode artefact shares.
///
/// `audio process` names the upload master `<base>.processed.mp4`, and that
/// infix is deliberate: the artefact contract distinguishes it from the edited
/// master `<base>.mp4`. Nothing downstream carries it, though. Extracted audio
/// is `<base>.wav` and the transcript family is `<base>.srt` and friends.
///
/// Deriving later names from the input stem alone propagated `.processed`
/// through the whole chain, because the only input holding processed audio is
/// `<base>.processed.wav`. `episode validate` and the YouTube caption lookup
/// then looked for `<base>.srt` and found nothing, so a complete pipeline run
/// reported missing captions (F2). Artefact naming has to be decided in one
/// place, and this is it.
pub fn episode_base(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    stem.strip_suffix(".processed")
        .map_or(stem.clone(), str::to_owned)
}

/// `<base>.<extension>` beside `path`, using the shared episode basename.
pub fn artefact(path: &Path, extension: &str) -> PathBuf {
    directory_of(path).join(format!("{}.{extension}", episode_base(path)))
}

/// The directory holding `path`, as a usable relative path when there is none.
pub fn directory_of(path: &Path) -> PathBuf {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

pub fn tilde_pathbuf(raw: &str) -> Result<PathBuf, String> {
    if raw == "~" || raw.starts_with("~/") {
        let home = std::env::var_os("HOME")
            .ok_or_else(|| "cannot expand '~' because HOME is not set".to_owned())?;
        let mut path = PathBuf::from(home);
        if raw.len() > 2 {
            path.push(&raw[2..]);
        }
        Ok(path)
    } else {
        Ok(PathBuf::from(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::{artefact, episode_base, tilde_pathbuf};
    use std::path::{Path, PathBuf};

    #[test]
    fn preserves_normal_paths() {
        assert_eq!(
            tilde_pathbuf("relative/file").unwrap().to_string_lossy(),
            "relative/file"
        );
    }

    #[test]
    fn the_processed_infix_never_reaches_a_later_artefact() {
        // Both the edited master and the upload master must name the same
        // episode, since the pipeline runs on the processed one.
        for input in [
            "/e/2026.09.15-show.mp4",
            "/e/2026.09.15-show.processed.mp4",
            "/e/2026.09.15-show.processed.wav",
        ] {
            assert_eq!(episode_base(Path::new(input)), "2026.09.15-show", "{input}");
        }
    }

    #[test]
    fn artefacts_land_beside_their_input_under_the_episode_name() {
        assert_eq!(
            artefact(Path::new("/e/show.processed.wav"), "srt"),
            PathBuf::from("/e/show.srt")
        );
        assert_eq!(
            artefact(Path::new("show.processed.mp4"), "wav"),
            PathBuf::from("./show.wav")
        );
    }
}
