#Requires -Version 5.1
<#
.SYNOPSIS
  Authenticode-sign Alamut binaries for 'Dashti Technologies LLC' + emit SHA256 sidecars.

.DESCRIPTION
  Signs each file in -Files with an Authenticode code-signing certificate whose
  subject matches 'Dashti Technologies LLC', using SHA256 + RFC3161 timestamp.
  Always emits a '<file>.sha256' sidecar (GNU-compatible "<HASH>  <name>" line)
  so distributors and AV allow-list submissions can prove integrity.

  Signing is best-effort by default: if no cert or signtool is found the script
  warns, skips signing, and still writes hashes (exit 0). Pass -RequireSignature
  in CI/release to fail instead of skipping.

  Certificate sources (in order):
    1. PFX file: -PfxPath or $env:ALAMUT_PFX_PATH, password in $env:ALAMUT_PFX_PASSWORD
    2. Cert store: CurrentUser\My then LocalMachine\My, subject contains -CertSubject,
       valid dates + code-signing EKU (1.3.6.1.5.5.7.3.3), newest first.

.EXAMPLE
  .\scripts\Sign-Binaries.ps1 -Files .\release\staging\alamut-api.exe, .\release\NetvaSetup.exe
  .\scripts\Sign-Binaries.ps1 -Files $exe -RequireSignature
  $env:ALAMUT_PFX_PATH='C:\certs\dashti.pfx'; .\scripts\Sign-Binaries.ps1 -Files $exe
#>

[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string[]]$Files,

  [string]$CertSubject = 'Dashti Technologies LLC',

  [string]$PfxPath = $env:ALAMUT_PFX_PATH,

  [string]$TimestampServer = 'http://timestamp.digicert.com',

  [string]$Description = 'Alamut by Dashti Technologies LLC',

  [string]$DescriptionUrl = '',

  [switch]$RequireSignature,

  [switch]$HashOnly
)

$ErrorActionPreference = 'Stop'

function Find-Signtool {
  if ($env:ALAMUT_SIGNTOOL -and (Test-Path -LiteralPath $env:ALAMUT_SIGNTOOL)) {
    return $env:ALAMUT_SIGNTOOL
  }
  try {
    $cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
  } catch { }
  $roots = @(
    ${env:ProgramFiles(x86)},
    $env:ProgramFiles
  ) | Where-Object { $_ } | Select-Object -Unique
  $candidates = foreach ($root in $roots) {
    $kits = Join-Path $root 'Windows Kits\10\bin'
    if (Test-Path -LiteralPath $kits) {
      Get-ChildItem -LiteralPath $kits -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending |
        ForEach-Object {
          $p = Join-Path $_.FullName 'x64\signtool.exe'
          if (Test-Path -LiteralPath $p) { $p }
        }
    }
    $vs = Join-Path $root 'Microsoft Visual Studio'
    if (Test-Path -LiteralPath $vs) {
      Get-ChildItem -LiteralPath $vs -Recurse -Filter 'signtool.exe' -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty FullName -First 1
    }
  }
  return ($candidates | Select-Object -First 1)
}

function Find-StoreCert([string]$subject) {
  $ekuCodeSigning = '1.3.6.1.5.5.7.3.3'
  foreach ($store in @('Cert:\CurrentUser\My', 'Cert:\LocalMachine\My')) {
    if (-not (Test-Path $store)) { continue }
    $certs = Get-ChildItem $store -ErrorAction SilentlyContinue |
      Where-Object {
        $_.Subject -like "*$subject*" -and
        $_.NotBefore -le (Get-Date) -and $_.NotAfter -ge (Get-Date) -and
        $_.HasPrivateKey
      }
    $signing = foreach ($c in $certs) {
      $ekus = @($c.Extensions | Where-Object { $_ -is [System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension] } |
        ForEach-Object { $_.EnhancedKeyUsages } | ForEach-Object { $_.Value })
      if ($ekus.Count -eq 0 -or ($ekus -contains $ekuCodeSigning)) { $c }
    }
    $best = $signing | Sort-Object NotAfter -Descending | Select-Object -First 1
    if ($best) { return $best }
  }
  return $null
}

function Write-Sha256Sidecar([string]$file) {
  $resolved = (Resolve-Path -LiteralPath $file).Path
  $hash = (Get-FileHash -Path $resolved -Algorithm SHA256).Hash.ToLowerInvariant()
  $line = "$hash  $(Split-Path -Leaf $resolved)"
  $sidecar = "$resolved.sha256"
  # ASCII, LF, no BOM — compatible with sha256sum -c
  [System.IO.File]::WriteAllText($sidecar, "$line`n", [System.Text.Encoding]::ASCII)
  Write-Host "hash  $(Split-Path -Leaf $resolved)  $hash"
  return @{ File = $resolved; Hash = $hash; Sidecar = $sidecar }
}

$results = @()
$missing = @($Files | Where-Object { -not (Test-Path -LiteralPath $_) })
if ($missing.Count -gt 0) {
  throw "Files not found: $($missing -join ', ')"
}

if ($HashOnly) {
  foreach ($f in $Files) { $results += Write-Sha256Sidecar $f }
  return
}

$signtool = Find-Signtool
$usePfx = ($PfxPath -and (Test-Path -LiteralPath $PfxPath))
$cert = $null
if (-not $usePfx) {
  $cert = Find-StoreCert $CertSubject
}

if (-not $signtool) {
  $msg = 'signtool.exe not found. Install Windows SDK or VS Build Tools, or set $env:ALAMUT_SIGNTOOL.'
  if ($RequireSignature) { throw $msg } else { Write-Warning "$msg Skipping signing; writing hashes only." }
} elseif (-not $usePfx -and -not $cert) {
  $msg = "No code-signing certificate found for subject '*$CertSubject*'. Install the Dashti Technologies LLC cert in CurrentUser\My (or set ALAMUT_PFX_PATH)."
  if ($RequireSignature) { throw $msg } else { Write-Warning "$msg Skipping signing; writing hashes only." }
} else {
  foreach ($f in $Files) {
    $resolved = (Resolve-Path -LiteralPath $f).Path
    $args = @('sign', '/fd', 'SHA256', '/td', 'SHA256', '/tr', $TimestampServer, '/d', $Description)
    if ($DescriptionUrl) { $args += @('/du', $DescriptionUrl) }
    if ($usePfx) {
      $args += @('/f', (Resolve-Path -LiteralPath $PfxPath).Path)
      if ($env:ALAMUT_PFX_PASSWORD) { $args += @('/p', $env:ALAMUT_PFX_PASSWORD) }
    } else {
      $args += @('/n', $CertSubject, '/a')
    }
    $args += $resolved
    Write-Host "==> signing $(Split-Path -Leaf $resolved) ..."
    & $signtool @args
    if ($LASTEXITCODE -ne 0) {
      throw "signtool sign failed for $resolved (exit $LASTEXITCODE)"
    }
    Write-Host "==> verifying $(Split-Path -Leaf $resolved) ..."
    $verifyOut = & $signtool verify /pa /v $resolved 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
      if ($verifyOut -match 'not trusted by the trust provider') {
        Write-Warning ("Signature applied but the chain is not publicly trusted " +
          "(self-signed or private CA). Integrity is intact; SmartScreen/AV reputation " +
          "requires a public OV/EV code-signing cert for '$CertSubject'.")
      } else {
        Write-Host $verifyOut
        throw "signtool verify failed for $resolved (exit $LASTEXITCODE)"
      }
    }
  }
  if ($cert) {
    Write-Host "Signed with: $($cert.Subject) (expires $($cert.NotAfter.ToString('yyyy-MM-dd')))"
  } elseif ($usePfx) {
    Write-Host "Signed with PFX: $PfxPath"
  }
}

foreach ($f in $Files) { $results += Write-Sha256Sidecar $f }

# Combined manifest next to the first file, for release-folder verification.
$dir = Split-Path -Parent (Resolve-Path -LiteralPath $Files[0]).Path
$manifest = Join-Path $dir 'SHA256SUMS.txt'
$lines = foreach ($r in $results) { "$($r.Hash)  $(Split-Path -Leaf $r.File)" }
[System.IO.File]::WriteAllText($manifest, (($lines -join "`n") + "`n"), [System.Text.Encoding]::ASCII)
Write-Host "wrote $manifest"
