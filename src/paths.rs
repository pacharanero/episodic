// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

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
    use super::tilde_pathbuf;

    #[test]
    fn preserves_normal_paths() {
        assert_eq!(
            tilde_pathbuf("relative/file").unwrap().to_string_lossy(),
            "relative/file"
        );
    }
}
