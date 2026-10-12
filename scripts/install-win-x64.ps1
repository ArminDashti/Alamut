#Requires -Version 5.1
<#
.SYNOPSIS
  Build and install/update both the Alamut API and WebUI on Windows x64.

.DESCRIPTION
  Compatibility entry point for the standalone install-win-x64.ps1 name.
  The shared installer performs a release API build, a production WebUI build,
  and installs or updates the single Alamut Windows service that runs both.

  Requires an elevated PowerShell, Rust/Cargo, Node.js/npm, and .NET when the
  API thermal helper is enabled by its Cargo build script.

.EXAMPLE
  .\scripts\install-win-x64.ps1

.EXAMPLE
  .\scripts\install-win-x64.ps1 port set 18000
#>
[CmdletBinding(PositionalBinding = $false)]
param(
  [string]$ProjectRoot,
  [string]$StackName = 'alamut',
  [string]$ServiceName = 'Alamut',
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$CliArgs
)

$ErrorActionPreference = 'Stop'

$installer = Join-Path $PSScriptRoot 'install.ps1'
if (-not (Test-Path -LiteralPath $installer)) {
  throw "Shared installer not found: $installer"
}

# This entry point is intentionally a full rebuild; do not inherit optional
# development shortcuts from the calling shell.
$env:ALAMUT_INSTALL_SKIP_API_BUILD = $null
$env:ALAMUT_INSTALL_SKIP_WEBUI_BUILD = $null

$forwarded = @('-StackName', $StackName, '-ServiceName', $ServiceName)
if (-not [string]::IsNullOrWhiteSpace($ProjectRoot)) {
  $forwarded += @('-ProjectRoot', $ProjectRoot)
}
if ($CliArgs) {
  $forwarded += $CliArgs
}

& $installer @forwarded
if ($LASTEXITCODE -ne 0) {
  throw "Alamut API + WebUI installation failed ($LASTEXITCODE)"
}
