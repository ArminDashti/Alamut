#Requires -RunAsAdministrator
<#
.SYNOPSIS
  Rebuild Alamut release binary, remove the old Windows service, install and start the new one.

.DESCRIPTION
  Run this after any code change that affects the installed Alamut Windows service.
  The single "Alamut" service covers both the Web UI and API (http://alamut.local).
  Must be elevated (Administrator).
#>
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $RepoRoot

function Get-InstalledRustTargets {
  $raw = & rustup target list --installed 2>$null
  if ($LASTEXITCODE -ne 0 -or -not $raw) { return @() }
  return @($raw | ForEach-Object { "$_".Trim() } | Where-Object { $_ })
}

function Resolve-ApiCargoTarget {
  $installed = Get-InstalledRustTargets
  $gnu = 'x86_64-pc-windows-gnu'
  $msvc = 'x86_64-pc-windows-msvc'
  if ($installed -contains $gnu) { return $gnu }
  if ($installed -contains $msvc) { return $msvc }
  return $msvc
}

function Find-ApiBinary {
  param([Parameter(Mandatory = $true)][string]$Target)
  # Prefer the binary cargo just built for $Target; target\release may be a stale host build.
  $candidates = @(
    (Join-Path $RepoRoot "target\$Target\release\alamut-api.exe"),
    (Join-Path $RepoRoot 'target\release\alamut-api.exe')
  )
  foreach ($c in $candidates) {
    if (Test-Path -LiteralPath $c) { return (Resolve-Path -LiteralPath $c).Path }
  }
  return $null
}

function Invoke-BestEffort {
  param(
    [Parameter(Mandatory = $true)][string]$ExePath,
    [Parameter(Mandatory = $true)][string[]]$ExeArgs
  )
  $prevEA = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    & $ExePath @ExeArgs 2>&1 | Out-Null
    return $LASTEXITCODE
  } catch {
    return 1
  } finally {
    $ErrorActionPreference = $prevEA
  }
}

$target = Resolve-ApiCargoTarget
Write-Host "==> cargo build -p alamut-api --release --target $target"
cargo build -p alamut-api --release --target $target
if ($LASTEXITCODE -ne 0) {
  throw "cargo build failed with exit code $LASTEXITCODE"
}

$Exe = Find-ApiBinary -Target $target
if (-not $Exe) {
  throw "Missing binary under target/*/release/alamut-api.exe"
}

Write-Host "==> stop (ignore if not running)"
Invoke-BestEffort -ExePath $Exe -ExeArgs @("stop") | Out-Null

Write-Host "==> uninstall old service"
$uninstallCode = Invoke-BestEffort -ExePath $Exe -ExeArgs @("uninstall")
if ($uninstallCode -ne 0) {
  # Not installed yet is OK on first install
  Write-Host "uninstall exited $uninstallCode (continuing if service was absent)"
}

Write-Host "==> install new service from $Exe"
& $Exe install
if ($LASTEXITCODE -ne 0) {
  throw "install failed with exit code $LASTEXITCODE"
}

Write-Host "==> start"
& $Exe start
if ($LASTEXITCODE -ne 0) {
  throw "start failed with exit code $LASTEXITCODE"
}

Write-Host "==> status"
& $Exe status

Write-Host "Done: old service removed, new release installed and started."
