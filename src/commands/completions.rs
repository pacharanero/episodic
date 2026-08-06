// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use anyhow::{Context, Result};
use clap::Command;
use clap_complete::{Shell, generate, generate_to};
use std::io;
use std::path::Path;

pub fn run(
    shell: Shell,
    directory: Option<&Path>,
    dry_run: bool,
    mut command: Command,
) -> Result<()> {
    if let Some(directory) = directory {
        if dry_run {
            eprintln!(
                "Would write {shell} completion for eps to {}",
                directory.display()
            );
            return Ok(());
        }
        std::fs::create_dir_all(directory).with_context(|| {
            format!(
                "could not create completion directory {}",
                directory.display()
            )
        })?;
        let path = generate_to(shell, &mut command, "eps", directory)
            .context("could not generate completion file")?;
        println!("{}", path.display());
    } else {
        generate(shell, &mut command, "eps", &mut io::stdout());
    }
    Ok(())
}
