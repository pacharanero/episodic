# Security Policy

## Reporting A Vulnerability

Do not disclose suspected vulnerabilities publicly before they have been assessed. Report them privately to Marcus Baw at <marcusbaw@gmail.com> with the affected version or commit, reproduction steps, impact, and any suggested mitigation.

Do not include real credentials, OAuth tokens, private episode media, unpublished guest details, or patient-identifiable information in a report. Replace secrets and sensitive data with clearly marked placeholders.

## Scope

Security-sensitive areas include path traversal outside configured workspaces, unsafe command invocation, credential leakage, remote writes without explicit approval, publication-state confusion, and destructive cleanup. Reports about third-party tools such as FFmpeg, Whisper, ImageMagick, language-model providers, or `yutu` should normally be sent to their maintainers unless `episodic` uses them unsafely.

## Supported Versions

The project is pre-release. Security fixes currently apply to the latest development version only.
