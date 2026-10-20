# Getting Started

`episodic` is a local-first CLI. A channel workspace is an ordinary directory containing `eps.toml`; episode records and generated artefacts remain under that directory.

## Install `eps`

Use one of these installation paths:

```bash
# Cargo installs from crates.io.
cargo install episodic-cli

# cargo-binstall downloads the matching release archive.
cargo binstall episodic-cli
```

On macOS or Linux, the release installer needs `curl` but not a Rust toolchain:

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/pacharanero/episodic/releases/latest/download/episodic-cli-installer.sh | sh
```

On Windows PowerShell:

```powershell
irm https://github.com/pacharanero/episodic/releases/latest/download/episodic-cli-installer.ps1 | iex
```

Each GitHub release also includes an MSI for Windows and prebuilt archives for macOS, Linux, and Windows. Check the installed command with `eps --version`.

## Create a channel workspace

Create a directory for one channel and add the smallest valid configuration:

```bash
mkdir my-channel && cd my-channel
cat > eps.toml <<'EOF'
schema_version = 1

[channel]
id = "my-channel"
name = "My Channel"
timezone = "Europe/London"
EOF
```

`eps` selects an explicit `--config <path>` first, otherwise `eps.toml` in the current directory. It never searches parent directories, environment variables, or global locations, so commands cannot silently select another channel.

For a production configuration, start from [`channels/example.toml`](../channels/example.toml) and use the [channel configuration reference](../channels/README.md) to add only the policy and integrations you need.

## Check the local environment

```bash
eps doctor
eps channel validate
```

`eps doctor` reports the selected workspace and the configured external tools without executing them, reading credentials, or contacting services. It tells you whether FFmpeg, Whisper, ImageMagick, model CLIs, prompt files, fonts, and YouTube credential locations are present when those are configured. It cannot confirm tool versions, model availability, or credential validity.

Install only the tools required by your workflow: FFmpeg for audio and frames, Whisper for transcription, ImageMagick for thumbnails, and a language-model CLI when generated metadata uses `provider = "cli"`.

## Create the first episode

```bash
eps episode new 2026-09-15 'First episode'
eps episode list
```

The command creates a planning-stage episode directory with show notes and a metadata skeleton. Continue with the [first-episode guide](first-episode.md) when media is ready.

## Set up your shell

Install completions without changing shell startup files:

```bash
eps completions install
```

Use `eps completions install --shell zsh` to select a shell, or `eps completions bash|zsh|fish|powershell|elvish` to print a script for a custom setup.
