#Requires -Version 5.1
<#
.SYNOPSIS
  Verify SHA256 sidecars (.sha256 or SHA256SUMS.txt) for Alamut binaries.

.DESCRIPTION
  Recomputes SHA256 for each target file and compares against its sidecar hash.
  Fails (exit 1) on any mismatch or missing sidecar. Use before installing or
  after downloading to prove the binary was not corrupted or tampered with.

.EXAMPLE
  .\scripts\Verify-Hash.ps1 -Files .\release\staging\alamut-api.exe
  .\scripts\Verify-Hash.ps1 -Manifest .\release\staging\SHA256SUMS.txt
#>

[CmdletBinding(DefaultParameterSetName = 'Files')]
param(
  [Parameter(ParameterSetName = 'Files', Mandatory = $true)]
  [string[]]$Files,

  [Parameter(ParameterSetName = 'Manifest', Mandatory = $true)]
  [string]$Manifest
)

$ErrorActionPreference = 'Stop'

function Test-One([string]$file, [string]$expected) {
  $resolved = (Resolve-Path -LiteralPath $file).Path
  $actual = (Get-FileHash -Path $resolved -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actual -ne $expected.ToLowerInvariant()) {
    Write-Host "[FAIL] $(Split-Path -Leaf $resolved)`n  expected: $expected`n  actual:   $actual"
    return $false
  }
  Write-Host "[OK] $(Split-Path -Leaf $resolved)  $actual"
  return $true
}

$ok = $true
if ($PSCmdlet.ParameterSetName -eq 'Manifest') {
  $dir = Split-Path -Parent (Resolve-Path -LiteralPath $Manifest).Path
  foreach ($line in (Get-Content -LiteralPath $Manifest)) {
    if (-not $line.Trim()) { continue }
    $parts = $line -split '\s+', 2
    if ($parts.Count -ne 2) { Write-Warning "Skipping malformed line: $line"; continue }
    $target = Join-Path $dir $parts[1].Trim()
    if (-not (Test-Path -LiteralPath $target)) {
      Write-Host "[FAIL] missing file: $($parts[1])"
      $ok = $false
      continue
    }
    if (-not (Test-One $target $parts[0])) { $ok = $false }
  }
} else {
  foreach ($f in $Files) {
    $resolved = (Resolve-Path -LiteralPath $f).Path
    $sidecar = "$resolved.sha256"
    if (-not (Test-Path -LiteralPath $sidecar)) {
      Write-Host "[FAIL] missing sidecar: $sidecar (run scripts/Sign-Binaries.ps1 -HashOnly)"
      $ok = $false
      continue
    }
    $line = (Get-Content -LiteralPath $sidecar -First 1).Trim()
    $expected = ($line -split '\s+')[0]
    if (-not (Test-One $resolved $expected)) { $ok = $false }
  }
}

if (-not $ok) { exit 1 }
Write-Host 'All hashes verified.'
