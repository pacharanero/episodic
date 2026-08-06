// SPDX-FileCopyrightText: 2026 Dr Marcus Baw and Koloki Ltd
// SPDX-License-Identifier: AGPL-3.0-or-later

#[cfg(unix)]
fn reset_sigpipe() {
    // SAFETY: Restoring SIGPIPE's default disposition is process-global startup
    // configuration and runs before the CLI starts any worker threads.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

fn main() {
    reset_sigpipe();
    if let Err(error) = episodic::cli::run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
