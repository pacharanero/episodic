// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::output::{self, OutputFormat};
use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct VersionInfo {
    name: &'static str,
    version: &'static str,
}

pub fn run(format: OutputFormat) -> Result<()> {
    let version = VersionInfo {
        name: "eps",
        version: env!("CARGO_PKG_VERSION"),
    };
    output::write(format, &version, &format!("eps {}", version.version))
}
