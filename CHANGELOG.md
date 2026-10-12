# Changelog

## [Unreleased]

### Added
- Embed Windows version resource (`CompanyName=Dashti Technologies LLC`,
  product metadata, file/product version) into `alamut-api.exe` and
  `alamut-cli.exe` via `winres` build scripts.
- Add `scripts/Sign-Binaries.ps1` (Authenticode SHA256 + RFC3161 timestamp
  for Dashti Technologies LLC + SHA256 sidecars/manifest) and
  `scripts/Verify-Hash.ps1`; hook best-effort sign+hash into
  `scripts/install-win-x64.ps1`. See `docs/code-signing.md`.

## [0.2.0] - 2026-09-27

### Added
- Add the Alamut terminal management CLI with app, network, and system views.

### Changed
- Expand CLI documentation and update workspace dependency metadata.
