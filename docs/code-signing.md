# Code signing & hashing — Alamut (Dashti Technologies LLC)

## What this does

1. **Version resource (build time):** `alamut-api` / `alamut-cli` embed
   `CompanyName=Dashti Technologies LLC`, product name, file description,
   copyright, and file/product version via `winres` in `build.rs`.
   Unsigned EXEs with empty CompanyName/FileVersion are a top SmartScreen /
   Defender heuristic trigger — this fixes that baseline.
2. **Authenticode signing (release time):** `scripts/Sign-Binaries.ps1` signs
   EXEs with the company code-signing cert (SHA256 + RFC3161 timestamp via
   DigiCert by default) and verifies with `signtool verify /pa /v`.
3. **SHA256 sidecars (always):** every signed (or hash-only) file gets
   `<file>.sha256` (`"<hash>  <name>"`, `sha256sum`-compatible) plus a
   `SHA256SUMS.txt` manifest in the release folder. Verify with
   `scripts/Verify-Hash.ps1`.

## One-time setup

1. Buy/renew an **OV or EV code-signing certificate** for
   **Dashti Technologies LLC** (EV gives instant SmartScreen reputation;
   OV builds reputation over time). Export as PFX or install in
   `CurrentUser\My` on the build machine.
2. Install **Windows SDK or VS Build Tools** so `signtool.exe` exists, or set:
   `$env:ALAMUT_SIGNTOOL='C:\path\to\signtool.exe'`
3. For PFX flows (CI):
   `$env:ALAMUT_PFX_PATH='C:\certs\dashti.pfx'`
   `$env:ALAMUT_PFX_PASSWORD='<secret>'` (never commit this).

## Usage

```powershell
# Release: sign + hash (fails if no cert — use in CI)
.\scripts\Sign-Binaries.ps1 -Files .\release\staging\alamut-api.exe, .\release\NetvaSetup.exe -RequireSignature

# Dev machine without cert: hashes only, never fails
.\scripts\Sign-Binaries.ps1 -Files .\release\staging\alamut-api.exe

# Verify later / on customer machine
.\scripts\Verify-Hash.ps1 -Files .\release\staging\alamut-api.exe
.\scripts\Verify-Hash.ps1 -Manifest .\release\staging\SHA256SUMS.txt
```

`scripts/install-win-x64.ps1` already calls `Sign-Binaries.ps1` best-effort
after copying `Alamut.exe`, so installs always leave a `.sha256` next to the
binary even when the build box has no cert.

## Current machine status (verified 2026-09-30)

- Windows SDK 10.0.26100.0 present: `signtool.exe` + `rc.exe` under
  `C:\Program Files (x86)\Windows Kits\10\bin\`.
- Cert store contains `CN=Dashti Technologies LLC (Armin Dashti)` (expires
  2036-09-01) — **self-signed** (issuer = subject). Signing works and the
  DigiCert timestamp applies, but the chain is not publicly trusted, so
  SmartScreen/Defender still treat the binary as unknown publisher.
- Action needed for real AV/SmartScreen benefit: import a **public CA-issued
  OV or EV code-signing cert** for Dashti Technologies LLC into
  `CurrentUser\My` (or point `ALAMUT_PFX_PATH` at its PFX). No script changes
  needed — `Sign-Binaries.ps1` picks it up automatically (newest valid
  code-signing cert matching the subject wins).

## Honest AV expectations

- Signing + metadata + timestamp **reduce** false positives (SmartScreen,
  Defender SmartScreen reputation, allow-list submissions) but **cannot
  guarantee** zero AV flags. Reputation builds with signed prevalence over time.
- The `.sha256` sidecar does **not** stop AV by itself — it lets users,
  support, and AV vendors verify a flagged file is the genuine published
  build (compare hash, submit to vendor false-positive portals with the
  signed sample + manifest).
- If Defender flags a build: confirm the hash matches `SHA256SUMS.txt`,
  check the signature tab shows `Dashti Technologies LLC`, then submit the
  signed file to the vendor as a false positive with version + hash.
