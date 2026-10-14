#Requires -Version 5.1
<#
.SYNOPSIS
  Alamut CLI - manage the Alamut service + headless stats (works despite the GUI).

.DESCRIPTION
  Global CLI wrapper. Service commands delegate to alamut-api.exe;
  stats commands (overview/cpu/memory/disk/network/os/machine/live)
  delegate to alamut-cli.exe (JSON-RPC, with local fallback when the
  API is down). Native wrapper commands: doctor/version/remove/update/webui/help.

.EXAMPLE
  alamut service status
  alamut overview
  alamut cpu live
  alamut machine info
  alamut live
  alamut webui
#>

[CmdletBinding()]
param(
  [Parameter(Position = 0)]
  [string]$Command,

  [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
  [string[]]$Args
)

$ErrorActionPreference = 'Stop'

# Resolve paths
$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = $ScriptRoot  # launcher lives in repo root next to netvan-api / netvan-webui
$ApiRoot = Join-Path $ProjectRoot 'netvan-api'

function Get-AlamutApiExe {
  $candidates = @(
    (Join-Path $ApiRoot 'target\release\alamut-api.exe'),
    (Join-Path $ApiRoot 'target\x86_64-pc-windows-gnu\release\alamut-api.exe'),
    (Join-Path $ApiRoot 'target\x86_64-pc-windows-msvc\release\alamut-api.exe')
  )
  foreach ($c in $candidates) {
    if (Test-Path -LiteralPath $c) { return (Resolve-Path -LiteralPath $c).Path }
  }
  # Installed copy?
  $installed = Join-Path $env:LOCALAPPDATA 'Alamut\Alamut.exe'
  if (Test-Path -LiteralPath $installed) { return $installed }
  return $null
}

function Get-AlamutCliExe {
  $candidates = @(
    (Join-Path $ApiRoot 'target\release\alamut-cli.exe'),
    (Join-Path $ApiRoot 'target\debug\alamut-cli.exe'),
    (Join-Path $ApiRoot 'target\x86_64-pc-windows-gnu\release\alamut-cli.exe'),
    (Join-Path $ApiRoot 'target\x86_64-pc-windows-msvc\release\alamut-cli.exe')
  )
  foreach ($c in $candidates) {
    if (Test-Path -LiteralPath $c) { return (Resolve-Path -LiteralPath $c).Path }
  }
  return $null
}

function Get-WrapperVersion {
  try {
    $toml = Join-Path $ApiRoot 'Cargo.toml'
    if (Test-Path -LiteralPath $toml) {
      $m = Select-String -Path $toml -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
      if ($m -and $m.Matches.Count -gt 0) { return $m.Matches[0].Groups[1].Value }
    }
  } catch { }
  return '0.2.0'
}

function Get-ApiBase {
  if ($env:ALAMUT_API_URL -and $env:ALAMUT_API_URL.Trim()) { return $env:ALAMUT_API_URL.Trim() }
  if ($env:ALAMUT_API_BASE -and $env:ALAMUT_API_BASE.Trim()) { return $env:ALAMUT_API_BASE.Trim() }
  if ($env:ALAMUT_API_BIND -and $env:ALAMUT_API_BIND.Trim()) { return ("http://" + $env:ALAMUT_API_BIND.Trim()) }
  return 'http://127.0.0.1:80'
}

function Show-Help {
  $v = Get-WrapperVersion
  Write-Host "◆ ALAMUT CLI v$v - service + headless stats (works despite the GUI)"
  Write-Host ""
  Write-Host "Usage: alamut <command> [args]"
  Write-Host ""
  Write-Host "Service:"
  Write-Host "  service status    Show service install/running status"
  Write-Host "  service start     Start the Alamut Windows service"
  Write-Host "  service stop      Stop the Alamut Windows service"
  Write-Host "  service restart   Restart the Alamut Windows service"
  Write-Host "  install           Install the Windows service"
  Write-Host "  run               Run API in foreground (dev/console mode)"
  Write-Host ""
  Write-Host "App:"
  Write-Host "  doctor            Run diagnostics and checks"
  Write-Host "  version           Print CLI + service binary versions"
  Write-Host "  remove [--purge]  Stop + uninstall service (purge also deletes data dir)"
  Write-Host "  update            Rebuild API+CLI+WebUI and restart the service if running"
  Write-Host "  rebuild           Alias for update"
  Write-Host "  webui             Open the Web UI in the default browser"
  Write-Host "  help              Show this help message"
  Write-Host ""
  Write-Host "Stats (one-shot, add --json for automation):"
  Write-Host "  overview          Service + cpu + memory + disks + NICs"
  Write-Host "  cpu               CPU snapshot (brand, load, per-core)"
  Write-Host "  memory            Memory snapshot"
  Write-Host "  disk              Disk volumes"
  Write-Host "  network           NIC table (link, IPv4, live rx/tx)"
  Write-Host "  os                OS / host / API reachability"
  Write-Host "  machine info      Static hardware inventory (cpu/mem/disks/gpus/board)"
  Write-Host ""
  Write-Host "Live (streaming, Ctrl+C stops, --interval-ms N):"
  Write-Host "  cpu live          Stream CPU load"
  Write-Host "  memory live       Stream memory usage"
  Write-Host "  disk live         Stream disk usage"
  Write-Host "  network live      Stream NIC throughput"
  Write-Host "  live              Full live view (cpu + memory + network)"
  Write-Host "  tui               Interactive colorful terminal dashboard"
  Write-Host ""
  Write-Host "Examples:"
  Write-Host "  alamut service status"
  Write-Host "  alamut overview --json"
  Write-Host "  alamut cpu live"
  Write-Host "  alamut machine info"
  Write-Host "  alamut live"
  Write-Host "  alamut webui"
}

function Invoke-Doctor {
  Write-Host "◆ ALAMUT Doctor - running diagnostics..."
  Write-Host ""
  $fail = 0

  $apiExe = Get-AlamutApiExe
  if ($apiExe) {
    Write-Host "[OK] alamut-api.exe found: $apiExe" -ForegroundColor Green
  } else {
    Write-Host "[FAIL] alamut-api.exe not found. Run: cd netvan-api; cargo build --release" -ForegroundColor Red
    $fail++
  }

  $cliExe = Get-AlamutCliExe
  if ($cliExe) {
    Write-Host "[OK] alamut-cli.exe found: $cliExe" -ForegroundColor Green
  } else {
    Write-Host "[INFO] alamut-cli.exe not found. Run: cd netvan-api; cargo build --release -p alamut-cli" -ForegroundColor Yellow
  }

  $svc = Get-Service -Name 'Alamut' -ErrorAction SilentlyContinue
  if ($svc) {
    Write-Host "[OK] Service 'Alamut' found (Status: $($svc.Status))" -ForegroundColor Green
  } else {
    Write-Host "[INFO] Service 'Alamut' not installed" -ForegroundColor Yellow
  }

  $dataDir = Join-Path $env:ProgramData 'Alamut\AlamutApi'
  if (Test-Path -LiteralPath $dataDir) {
    Write-Host "[OK] Data directory exists: $dataDir" -ForegroundColor Green
  } else {
    Write-Host "[INFO] Data directory not found: $dataDir" -ForegroundColor Yellow
  }

  $webuiDist = Join-Path $ProjectRoot 'netvan-webui\dist'
  if ((Test-Path -LiteralPath (Join-Path $webuiDist 'index.html'))) {
    Write-Host "[OK] Web UI build found: $webuiDist" -ForegroundColor Green
  } else {
    Write-Host "[INFO] Web UI not built. Run: cd netvan-webui; npm install; npm run build" -ForegroundColor Yellow
  }

  # API health
  $base = Get-ApiBase
  try {
    $h = Invoke-RestMethod -Uri "$base/api/health" -TimeoutSec 5 -ErrorAction Stop
    if ($h.ok -eq $true -or $h.service -eq 'Alamut') {
      Write-Host "[OK] API reachable at $base" -ForegroundColor Green
    } else {
      Write-Host "[INFO] API responded oddly at $base" -ForegroundColor Yellow
    }
  } catch {
    Write-Host "[INFO] API not reachable at $base (start it: alamut run / alamut service start)" -ForegroundColor Yellow
  }

  # hosts entry for alamut.local
  try {
    $hosts = Get-Content "$env:SystemRoot\System32\drivers\etc\hosts" -ErrorAction Stop
    if ($hosts -match 'alamut\.local') {
      Write-Host "[OK] hosts entry for alamut.local present" -ForegroundColor Green
    } else {
      Write-Host "[INFO] hosts entry missing: add '127.0.0.1  alamut.local'" -ForegroundColor Yellow
    }
  } catch {
    Write-Host "[INFO] could not read hosts file" -ForegroundColor Yellow
  }

  Write-Host ""
  if ($fail -eq 0) { Write-Host "Doctor complete." -ForegroundColor Green } else { Write-Host "Doctor complete with $fail failure(s)." -ForegroundColor Red }
  return $fail
}

function Invoke-Version {
  $v = Get-WrapperVersion
  Write-Host "◆ ALAMUT v$v"
  $apiExe = Get-AlamutApiExe
  if ($apiExe) {
    try {
      $out = & $apiExe version 2>$null
      if ($LASTEXITCODE -eq 0 -and $out) { Write-Host "  $out" } else { Write-Host "  alamut-api: $apiExe" }
    } catch { Write-Host "  alamut-api: $apiExe" }
  } else {
    Write-Host "  alamut-api: not built"
  }
  $cliExe = Get-AlamutCliExe
  if ($cliExe) {
    try {
      $out = & $cliExe version 2>$null
      if ($LASTEXITCODE -eq 0 -and $out) { Write-Host "  $out" } else { Write-Host "  alamut-cli: $cliExe" }
    } catch { Write-Host "  alamut-cli: $cliExe" }
  } else {
    Write-Host "  alamut-cli: not built"
  }
}

function Invoke-Remove {
  param([string[]]$Extra)
  $purge = $Extra -contains '--purge'
  Write-Host "◆ ALAMUT remove - stopping + uninstalling service..."
  $apiExe = Get-AlamutApiExe
  if (-not $apiExe) {
    Write-Host "Error: alamut-api.exe not found. Nothing to uninstall via binary."
    Write-Host "  cd netvan-api"
    Write-Host "  cargo build --release"
    exit 1
  }
  & $apiExe stop
  & $apiExe uninstall
  $code = $LASTEXITCODE
  if ($purge) {
    $dataDir = Join-Path $env:ProgramData 'Alamut\AlamutApi'
    if (Test-Path -LiteralPath $dataDir) {
      Write-Host "==> Purging data directory: $dataDir"
      Remove-Item -LiteralPath $dataDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    $installDir = Join-Path $env:LOCALAPPDATA 'Alamut'
    if (Test-Path -LiteralPath $installDir) {
      $answer = Read-Host "Delete install dir $installDir ? [y/N]"
      if ($answer -match '^[Yy]') {
        Remove-Item -LiteralPath $installDir -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "Deleted $installDir"
      }
    }
  } else {
    Write-Host "Tip: 'alamut remove --purge' also deletes the data directory."
  }
  exit $code
}

function Invoke-Update {
  Write-Host "◆ ALAMUT update - rebuilding API + CLI + WebUI..."
  $ApiDir = Join-Path $ProjectRoot 'netvan-api'
  $WebuiDir = Join-Path $ProjectRoot 'netvan-webui'
  $svc = Get-Service -Name 'Alamut' -ErrorAction SilentlyContinue
  $wasRunning = $svc -and ($svc.Status -eq 'Running')

  Push-Location $ApiDir
  try {
    cargo build --release -p alamut-api -p alamut-cli
    if ($LASTEXITCODE -ne 0) { Write-Host "cargo build failed." -ForegroundColor Red; exit 1 }
  } finally { Pop-Location }

  if ((Test-Path -LiteralPath (Join-Path $WebuiDir 'package.json'))) {
    Push-Location $WebuiDir
    try {
      if (Get-Command npm -ErrorAction SilentlyContinue) {
        if (-not (Test-Path -LiteralPath (Join-Path $WebuiDir 'node_modules'))) { npm install }
        npm run build
        if ($LASTEXITCODE -ne 0) { Write-Host "webui build failed." -ForegroundColor Red; exit 1 }
      } else {
        Write-Host "[INFO] npm not found - skipping WebUI build." -ForegroundColor Yellow
      }
    } finally { Pop-Location }
  }

  if ($wasRunning) {
    Write-Host "==> Restarting service..."
    $apiExe = Get-AlamutApiExe
    if ($apiExe) { & $apiExe stop; Start-Sleep -Seconds 2; & $apiExe start; exit $LASTEXITCODE }
  }
  Write-Host "Update complete." -ForegroundColor Green
  exit 0
}

function Invoke-WebUI {
  param([string[]]$Extra)
  $custom = $null
  for ($i = 0; $i -lt $Extra.Count; $i++) {
    if ($Extra[$i] -eq '--url' -and ($i + 1) -lt $Extra.Count) { $custom = $Extra[$i + 1] }
  }
  $url = $custom
  if (-not $url) {
    # Prefer the pretty hostname when the API answers there, else the API base.
    $url = 'http://alamut.local'
    try {
      $r = Invoke-WebRequest -Uri $url -Method Head -TimeoutSec 3 -UseBasicParsing -ErrorAction Stop
      if ($r.StatusCode -ge 400) { throw 'bad status' }
    } catch {
      $url = Get-ApiBase
    }
  }
  Write-Host "◆ Opening Web UI: $url"
  try { Start-Process $url } catch { Write-Host "Open manually: $url" }
  exit 0
}

function Invoke-Cli {
  param([string[]]$CliArgs)
  $cliExe = Get-AlamutCliExe
  if (-not $cliExe) {
    Write-Host "Error: alamut-cli.exe not found. Build it first:"
    Write-Host "  cd netvan-api"
    Write-Host "  cargo build --release -p alamut-cli"
    exit 1
  }
  # Hoist global flags (--plain, --api X, --poll-ms X) ahead of the subcommand.
  $globals = @()
  $rest = @()
  for ($i = 0; $i -lt $CliArgs.Count; $i++) {
    $a = $CliArgs[$i]
    if ($a -eq '--plain') { $globals += $a }
    elseif (($a -eq '--api' -or $a -eq '--poll-ms') -and ($i + 1) -lt $CliArgs.Count) {
      $globals += $a; $i++; $globals += $CliArgs[$i]
    }
    elseif ($a -like '--api=*') { $globals += $a }
    elseif ($a -like '--poll-ms=*') { $globals += $a }
    else { $rest += $a }
  }
  $final = $globals + $rest
  & $cliExe @final
  exit $LASTEXITCODE
}

function Invoke-Api {
  param([string[]]$ApiArgs)
  $apiExe = Get-AlamutApiExe
  if (-not $apiExe) {
    Write-Host "Error: alamut-api.exe not found. Build it first:"
    Write-Host "  cd netvan-api"
    Write-Host "  cargo build --release"
    exit 1
  }
  & $apiExe @ApiArgs
  exit $LASTEXITCODE
}

# ---- top-level aliases
if ([string]::IsNullOrWhiteSpace($Command) -or $Command -eq 'help' -or $Command -eq '--help' -or $Command -eq '-h') {
  Show-Help
  exit 0
}
if ($Command -eq 'version' -or $Command -eq '--version' -or $Command -eq '-v') {
  Invoke-Version
  exit 0
}

# ---- service subcommands
if ($Command -eq 'service') {
  if ($null -eq $Args -or $Args.Count -eq 0) {
    Write-Host "Error: service command requires an action (start, stop, restart, status)"
    Show-Help
    exit 1
  }
  $action = $Args[0]
  switch ($action) {
    'start'   { Invoke-Api @('start') }
    'stop'    { Invoke-Api @('stop') }
    'restart' {
      Write-Host "==> Restarting Alamut service..."
      $apiExe = Get-AlamutApiExe
      if (-not $apiExe) { Write-Host "Error: alamut-api.exe not found."; exit 1 }
      & $apiExe stop
      Start-Sleep -Seconds 2
      & $apiExe start
      exit $LASTEXITCODE
    }
    'status'  { Invoke-Api @('status') }
    default {
      Write-Host "Error: Unknown service action '$action'. Use: start, stop, restart, status"
      exit 1
    }
  }
}

# ---- dispatch
switch ($Command) {
  'tui'      { Invoke-Cli @('tui') }
  'overview' { Invoke-Cli (@('overview') + $Args) }
  'cpu'      { Invoke-Cli (@('cpu') + $Args) }
  'memory'   { Invoke-Cli (@('memory') + $Args) }
  'disk'     { Invoke-Cli (@('disk') + $Args) }
  'network'  { Invoke-Cli (@('network') + $Args) }
  'os'       { Invoke-Cli (@('os') + $Args) }
  'machine'  {
    # `alamut machine info` -> cli `machine-info` (extra word tolerated)
    Invoke-Cli (@('machine-info') + $Args)
  }
  'machine-info' { Invoke-Cli (@('machine-info') + $Args) }
  'live'     { Invoke-Cli (@('live') + $Args) }
  'webui'    { Invoke-WebUI -Extra $Args }
  'doctor'   { exit (Invoke-Doctor) }
  'remove'   { Invoke-Remove -Extra $Args }
  { $_ -in 'update', 'rebuild' } { Invoke-Update }
  'run'      { Invoke-Api (@('run') + $Args) }
  'install'  { Invoke-Api @('install') }
  'uninstall' { Invoke-Api @('uninstall') }
  default {
    Write-Host "Error: Unknown command '$Command'"
    Show-Help
    exit 1
  }
}
