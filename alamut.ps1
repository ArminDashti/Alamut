#Requires -Version 5.1
<#
.SYNOPSIS
  Alamut CLI - lifecycle for netvan-api (Windows service) and netvan-webui (static preview).

.DESCRIPTION
  Service commands delegate to alamut-api.exe; stats commands delegate to alamut-cli.exe.
  Lifecycle: run/stop/status/port/webui/reinstall/doctor/remove.
  WebUI state: %LOCALAPPDATA%\Alamut\{config.json,webui.pid}.

.EXAMPLE
  alamut run
  alamut status
  alamut port --port=8010
  alamut doctor --fix
  alamut reinstall --remove-data=no -y
#>

[CmdletBinding()]
param(
  [Parameter(Position = 0)]
  [string]$Command,

  [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
  [string[]]$Args
)

$ErrorActionPreference = 'Stop'

$ScriptRoot  = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = $ScriptRoot
$ApiRoot     = Join-Path $ProjectRoot 'netvan-api'
$WebuiRoot   = Join-Path $ProjectRoot 'netvan-webui'
$StateDir    = Join-Path $env:LOCALAPPDATA 'Alamut'
$ConfigFile  = Join-Path $StateDir 'config.json'
$WebuiPid    = Join-Path $StateDir 'webui.pid'
$DataDir     = Join-Path $env:ProgramData 'Alamut\AlamutApi'
$ServiceName = 'Alamut'
$DefaultWebPort = 8001
$ApiPort = 8000

# ---- helpers ---------------------------------------------------------------

function Test-Admin {
  $p = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
  return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-Flag {
  param([string[]]$Extra, [string]$Name)
  for ($i = 0; $i -lt $Extra.Count; $i++) {
    if ($Extra[$i] -eq "--$Name" -and ($i + 1) -lt $Extra.Count) { return $Extra[$i + 1] }
    if ($Extra[$i] -like "--$Name=*") { return $Extra[$i].Substring($Name.Length + 3) }
  }
  return $null
}

function Has-Switch {
  param([string[]]$Extra, [string[]]$Names)
  foreach ($n in $Names) { if ($Extra -contains "--$n" -or $Extra -contains "-$n") { return $true } }
  return $false
}

function Get-Config {
  $cfg = [pscustomobject]@{ port = $DefaultWebPort }
  if (Test-Path -LiteralPath $ConfigFile) {
    try {
      $raw = Get-Content -LiteralPath $ConfigFile -Raw | ConvertFrom-Json
      if ($raw.port) { $cfg.port = [int]$raw.port }
    } catch { }
  }
  return $cfg
}

function Save-Config {
  param($Cfg)
  New-Item -ItemType Directory -Force -Path $StateDir | Out-Null
  $Cfg | ConvertTo-Json | Set-Content -LiteralPath $ConfigFile -Encoding UTF8
}

# TCP connect probe: avoids Get-NetTCPConnection state-filter quirks on Windows.
function Test-PortListening {
  param([int]$Port)
  $client = New-Object System.Net.Sockets.TcpClient
  try {
    $ar = $client.BeginConnect('127.0.0.1', $Port, $null, $null)
    return ($ar.AsyncWaitHandle.WaitOne(500) -and $client.Connected)
  } catch { return $false } finally { $client.Close() }
}

function Get-WebuiPid {
  if (-not (Test-Path -LiteralPath $WebuiPid)) { return $null }
  $raw = (Get-Content -LiteralPath $WebuiPid -Raw).Trim()
  $id = 0
  if (-not [int]::TryParse($raw, [ref]$id)) { return $null }
  return $id
}

function Test-WebuiProcessAlive {
  $id = Get-WebuiPid
  if (-not $id) { return $false }
  return [bool](Get-Process -Id $id -ErrorAction SilentlyContinue)
}

function Test-WebuiRunning {
  return ((Test-WebuiProcessAlive) -and (Test-PortListening (Get-Config).port))
}

function Get-ServiceState {
  return Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
}

function Get-AlamutApiExe {
  $candidates = @(
    (Join-Path $ApiRoot 'target\release\alamut-api.exe'),
    (Join-Path $ApiRoot 'target\x86_64-pc-windows-gnu\release\alamut-api.exe'),
    (Join-Path $ApiRoot 'target\x86_64-pc-windows-msvc\release\alamut-api.exe')
  )
  foreach ($c in $candidates) {
    if (Test-Path -LiteralPath $c) { return (Resolve-Path -LiteralPath $c).Path }
  }
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
  return "http://127.0.0.1:$ApiPort"
}

function Wait-Http {
  param([string]$Url, [int]$Seconds = 30)
  $deadline = (Get-Date).AddSeconds($Seconds)
  while ((Get-Date) -lt $deadline) {
    try {
      $r = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 2
      if ($r.StatusCode -eq 200) { return $true }
    } catch { }
    Start-Sleep -Milliseconds 500
  }
  return $false
}

function Stop-ProcessTree {
  param([int]$ProcessId)
  & taskkill.exe /PID $ProcessId /T /F *> $null
}

# ---- WebUI -------------------------------------------------------------------

function Start-WebUi {
  $port = (Get-Config).port
  if (Test-WebuiRunning) {
    Write-Host "WebUI already running on http://127.0.0.1:$port"
    return 0
  }
  if (-not (Test-Path -LiteralPath (Join-Path $WebuiRoot 'dist\index.html'))) {
    Write-Host "==> WebUI not built; running npm run build..."
    Push-Location $WebuiRoot
    try {
      if (-not (Test-Path -LiteralPath 'node_modules')) { npm install | Out-Host }
      npm run build | Out-Host
      if ($LASTEXITCODE -ne 0) { Write-Host "webui build failed." -ForegroundColor Red; return 1 }
    } finally { Pop-Location }
  }
  if (Test-PortListening $port) {
    Write-Host "Error: port $port is in use by another process. Use 'alamut port --port=N'." -ForegroundColor Red
    return 1
  }
  Write-Host "==> Starting WebUI on http://127.0.0.1:$port ..."
  $proc = Start-Process -FilePath 'cmd.exe' -ArgumentList @('/c', 'npx', 'vite', 'preview', '--host', '127.0.0.1', '--port', "$port", '--strictPort') `
    -WorkingDirectory $WebuiRoot -WindowStyle Hidden -PassThru
  New-Item -ItemType Directory -Force -Path $StateDir | Out-Null
  Set-Content -LiteralPath $WebuiPid -Value $proc.Id -Encoding ASCII
  if (Wait-Http "http://127.0.0.1:$port/" 30) {
    Write-Host "WebUI up: http://127.0.0.1:$port" -ForegroundColor Green
    return 0
  }
  Write-Host "WebUI did not answer within 30s." -ForegroundColor Red
  Stop-WebUi | Out-Null
  return 1
}

function Stop-WebUi {
  $port = (Get-Config).port
  $id = Get-WebuiPid
  if ($id) { Stop-ProcessTree $id }
  if (Test-Path -LiteralPath $WebuiPid) { Remove-Item -LiteralPath $WebuiPid -Force }
  $deadline = (Get-Date).AddSeconds(10)
  while ((Test-PortListening $port) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 300 }
  if (Test-PortListening $port) {
    Write-Host "Error: port $port still listening after WebUI stop (not started by alamut)." -ForegroundColor Red
    return 1
  }
  Write-Host "WebUI stopped."
  return 0
}

function Invoke-Port {
  param([string[]]$Extra)
  $val = Get-Flag $Extra 'port'
  $cfg = Get-Config
  if (-not $val) { Write-Host "WebUI port: $($cfg.port)"; return 0 }
  $n = 0
  if (-not [int]::TryParse($val, [ref]$n) -or $n -lt 1 -or $n -gt 65535) {
    Write-Host "Error: port must be 1-65535." -ForegroundColor Red; return 2
  }
  if ($n -ne $cfg.port -and (Test-PortListening $n)) {
    Write-Host "Error: port $n is already in use." -ForegroundColor Red; return 1
  }
  $wasRunning = Test-WebuiRunning
  if ($wasRunning) { Stop-WebUi | Out-Null }
  $cfg.port = $n
  Save-Config $cfg
  Write-Host "WebUI port saved: $n"
  if ($wasRunning) { return (Start-WebUi) }
  return 0
}

function Invoke-WebUI {
  param([string[]]$Extra)
  $action = if ($Extra.Count -gt 0) { $Extra[0] } else { '' }
  switch ($action) {
    'run'  { exit (Start-WebUi) }
    'stop' { exit (Stop-WebUi) }
    default {
      $url = Get-Flag $Extra 'url'
      if (-not $url) {
        $url = "http://127.0.0.1:$((Get-Config).port)"
        if (-not (Test-WebuiRunning)) { $url = 'http://alamut.local' }
      }
      Write-Host "* Opening Web UI: $url"
      try { Start-Process $url } catch { Write-Host "Open manually: $url" }
      exit 0
    }
  }
}

# ---- lifecycle ---------------------------------------------------------------

function Invoke-Status {
  $svc = Get-ServiceState
  $port = (Get-Config).port
  if (-not $svc) {
    Write-Host "API service: not installed"
  } else {
    Write-Host "API service: $($svc.Status) (port $ApiPort)"
  }
  $webState = if (Test-WebuiRunning) { 'running' } else { 'stopped' }
  Write-Host "WebUI: $webState (http://127.0.0.1:$port)"
  if (-not $svc) { return 4 }
  if ($svc.Status -eq 'Running' -and (Test-WebuiRunning)) { return 0 }
  return 3
}

function Invoke-Run {
  $svc = Get-ServiceState
  if (-not $svc) {
    Write-Host "Error: API service not installed. Run: alamut install (elevated)." -ForegroundColor Red
    return 4
  }
  if ($svc.Status -ne 'Running') {
    if (-not (Test-Admin)) { Write-Host "Error: starting the service needs Administrator." -ForegroundColor Red; return 1 }
    & (Get-AlamutApiExe) start | Out-Host
    if ($LASTEXITCODE -ne 0) { return $LASTEXITCODE }
  }
  return (Start-WebUi)
}

function Invoke-Stop {
  $rc = Stop-WebUi
  $svc = Get-ServiceState
  if ($svc -and $svc.Status -eq 'Running') {
    if (-not (Test-Admin)) { Write-Host "Error: stopping the service needs Administrator." -ForegroundColor Red; return 1 }
    & (Get-AlamutApiExe) stop | Out-Host
    if ($LASTEXITCODE -ne 0) { $rc = $LASTEXITCODE }
  }
  return $rc
}

function Invoke-Reinstall {
  param([string[]]$Extra)
  if (-not (Test-Admin)) { Write-Host "Error: reinstall needs an elevated (Administrator) shell." -ForegroundColor Red; return 1 }
  $apiExe = Get-AlamutApiExe
  if (-not $apiExe) { Write-Host "Error: alamut-api.exe not built. cd netvan-api; cargo build --release" -ForegroundColor Red; return 1 }
  $removeData = Get-Flag $Extra 'remove-data'
  if (-not $removeData) { $removeData = 'no' }
  if ($removeData -notin 'yes', 'no') { Write-Host "Error: --remove-data must be yes or no." -ForegroundColor Red; return 2 }
  $wasRunning = (Get-ServiceState) -and ((Get-ServiceState).Status -eq 'Running')
  if ($removeData -eq 'yes') {
    if (-not (Has-Switch $Extra @('y'))) {
      $answer = Read-Host "Delete all API data in $DataDir ? [y/N]"
      if ($answer -notmatch '^[Yy]') { Write-Host "Aborted."; return 1 }
    }
  }
  Write-Host "==> Stopping WebUI and service..."
  Stop-WebUi | Out-Null
  if (Get-ServiceState) { & $apiExe stop | Out-Host; Start-Sleep -Seconds 2 }
  Write-Host "==> Uninstalling old service..."
  if (Get-ServiceState) { & $apiExe uninstall | Out-Host }
  if ($removeData -eq 'yes' -and (Test-Path -LiteralPath $DataDir)) {
    Remove-Item -LiteralPath $DataDir -Recurse -Force
    Write-Host "Deleted $DataDir"
  }
  Write-Host "==> Installing new service..."
  & $apiExe install | Out-Host
  if ($LASTEXITCODE -ne 0) { Write-Host "install failed." -ForegroundColor Red; return 1 }
  if ($wasRunning) {
    Write-Host "==> Starting service..."
    & $apiExe start | Out-Host
    if ($LASTEXITCODE -ne 0) { return $LASTEXITCODE }
  }
  Invoke-Status | Out-Null
  return 0
}

function Invoke-Remove {
  param([string[]]$Extra)
  $purge = Has-Switch $Extra @('purge')
  $yes = Has-Switch $Extra @('y')
  Write-Host "* ALAMUT remove - stopping WebUI and service..."
  if (-not $yes) {
    $answer = Read-Host "Remove service, WebUI state$(if ($purge) { ' and data' })? [y/N]"
    if ($answer -notmatch '^[Yy]') { Write-Host "Aborted."; exit 1 }
  }
  Stop-WebUi | Out-Null
  $apiExe = Get-AlamutApiExe
  if ((Get-ServiceState) -and $apiExe) {
    & $apiExe stop
    & $apiExe uninstall
  }
  if ($purge) {
    if (Test-Path -LiteralPath $DataDir) { Remove-Item -LiteralPath $DataDir -Recurse -Force }
    if (Test-Path -LiteralPath $StateDir) { Remove-Item -LiteralPath $StateDir -Recurse -Force }
  } elseif (Test-Path -LiteralPath $WebuiPid) {
    Remove-Item -LiteralPath $WebuiPid -Force
  }
  Write-Host "==> Leftover scan:"
  $left = 0
  if (Get-ServiceState) { Write-Host "  service '$ServiceName' still installed"; $left++ }
  foreach ($p in @($ApiPort, (Get-Config).port)) {
    if (Test-PortListening $p) { Write-Host "  port $p still listening"; $left++ }
  }
  if ($purge -and (Test-Path -LiteralPath $DataDir)) { Write-Host "  $DataDir remains"; $left++ }
  if ($purge -and (Test-Path -LiteralPath $StateDir)) { Write-Host "  $StateDir remains"; $left++ }
  if ($left -eq 0) { Write-Host "Clean." -ForegroundColor Green; exit 0 }
  exit 1
}

function Invoke-Doctor {
  param([string[]]$Extra)
  $fix = Has-Switch $Extra @('fix')
  Write-Host "* ALAMUT Doctor - running diagnostics$(if ($fix) { ' (with --fix)' })..."
  Write-Host ""
  $fail = 0

  if (Get-AlamutApiExe) { Write-Host "[OK] alamut-api.exe found" -ForegroundColor Green }
  else { Write-Host "[FAIL] alamut-api.exe not found. Run: cd netvan-api; cargo build --release" -ForegroundColor Red; $fail++ }

  if (Get-AlamutCliExe) { Write-Host "[OK] alamut-cli.exe found" -ForegroundColor Green }
  else { Write-Host "[INFO] alamut-cli.exe not found. Run: cd netvan-api; cargo build --release -p alamut-cli" -ForegroundColor Yellow }

  $svc = Get-ServiceState
  if ($svc) { Write-Host "[OK] Service '$ServiceName' found (Status: $($svc.Status))" -ForegroundColor Green }
  else { Write-Host "[INFO] Service '$ServiceName' not installed" -ForegroundColor Yellow }

  if (Test-Path -LiteralPath $DataDir) { Write-Host "[OK] Data directory exists: $DataDir" -ForegroundColor Green }
  else { Write-Host "[INFO] Data directory not found: $DataDir" -ForegroundColor Yellow }

  if (Test-Path -LiteralPath (Join-Path $WebuiRoot 'dist\index.html')) { Write-Host "[OK] WebUI build found" -ForegroundColor Green }
  else { Write-Host "[INFO] WebUI not built. Run: alamut webui run" -ForegroundColor Yellow }

  if (Test-Path -LiteralPath $ConfigFile) {
    try { $null = Get-Content -LiteralPath $ConfigFile -Raw | ConvertFrom-Json; Write-Host "[OK] config.json valid" -ForegroundColor Green }
    catch {
      Write-Host "[FAIL] config.json invalid: $ConfigFile" -ForegroundColor Red; $fail++
      if ($fix) { Remove-Item -LiteralPath $ConfigFile -Force; Write-Host "  fixed: reset config.json" -ForegroundColor Green; $fail-- }
    }
  }

  if (Test-Path -LiteralPath $WebuiPid) {
    if (Test-WebuiProcessAlive) { Write-Host "[OK] webui.pid process alive" -ForegroundColor Green }
    else {
      Write-Host "[FAIL] stale webui.pid (process gone)" -ForegroundColor Red; $fail++
      if ($fix) { Remove-Item -LiteralPath $WebuiPid -Force; Write-Host "  fixed: removed stale webui.pid" -ForegroundColor Green; $fail-- }
    }
  }

  if ($fix -and -not (Test-Path -LiteralPath $StateDir)) { New-Item -ItemType Directory -Force -Path $StateDir | Out-Null; Write-Host "  fixed: created $StateDir" -ForegroundColor Green }

  $base = Get-ApiBase
  try {
    $h = Invoke-RestMethod -Uri "$base/api/health" -TimeoutSec 5 -ErrorAction Stop
    if ($h.ok -eq $true -or $h.service -eq 'Alamut') { Write-Host "[OK] API reachable at $base" -ForegroundColor Green }
    else { Write-Host "[INFO] API responded oddly at $base" -ForegroundColor Yellow }
  } catch { Write-Host "[INFO] API not reachable at $base (start it: alamut run)" -ForegroundColor Yellow }

  Write-Host ""
  if ($fail -eq 0) { Write-Host "Doctor complete." -ForegroundColor Green } else { Write-Host "Doctor complete with $fail failure(s)." -ForegroundColor Red }
  return $fail
}

function Invoke-Version {
  $v = Get-WrapperVersion
  Write-Host "* ALAMUT v$v"
  $apiExe = Get-AlamutApiExe
  if ($apiExe) {
    try {
      $out = & $apiExe version 2>$null
      if ($LASTEXITCODE -eq 0 -and $out) { Write-Host "  $out" } else { Write-Host "  alamut-api: $apiExe" }
    } catch { Write-Host "  alamut-api: $apiExe" }
  } else { Write-Host "  alamut-api: not built" }
  $cliExe = Get-AlamutCliExe
  if ($cliExe) {
    try {
      $out = & $cliExe version 2>$null
      if ($LASTEXITCODE -eq 0 -and $out) { Write-Host "  $out" } else { Write-Host "  alamut-cli: $cliExe" }
    } catch { Write-Host "  alamut-cli: $cliExe" }
  } else { Write-Host "  alamut-cli: not built" }
  $dist = Join-Path $WebuiRoot 'dist\index.html'
  if (Test-Path -LiteralPath $dist) { Write-Host "  webui build: $((Get-Item $dist).LastWriteTime)" }
  else { Write-Host "  webui build: not built" }
}

function Invoke-Update {
  Write-Host "* ALAMUT update - rebuilding API + CLI + WebUI..."
  $wasRunning = (Get-ServiceState) -and ((Get-ServiceState).Status -eq 'Running')
  Push-Location $ApiRoot
  try {
    cargo build --release -p alamut-api -p alamut-cli
    if ($LASTEXITCODE -ne 0) { Write-Host "cargo build failed." -ForegroundColor Red; exit 1 }
  } finally { Pop-Location }
  Push-Location $WebuiRoot
  try {
    if (-not (Test-Path -LiteralPath 'node_modules')) { npm install }
    npm run build
    if ($LASTEXITCODE -ne 0) { Write-Host "webui build failed." -ForegroundColor Red; exit 1 }
  } finally { Pop-Location }
  if ($wasRunning) {
    Write-Host "==> Restarting service..."
    $apiExe = Get-AlamutApiExe
    if ($apiExe) { & $apiExe stop; Start-Sleep -Seconds 2; & $apiExe start; exit $LASTEXITCODE }
  }
  Write-Host "Update complete." -ForegroundColor Green
  exit 0
}

function Show-Help {
  $v = Get-WrapperVersion
  Write-Host "* ALAMUT CLI v$v - API service + WebUI lifecycle + headless stats"
  Write-Host ""
  Write-Host "Usage: alamut <command> [args]"
  Write-Host ""
  Write-Host "Lifecycle:"
  Write-Host "  run               Start API service + WebUI"
  Write-Host "  stop              Stop WebUI + API service"
  Write-Host "  status            Service + WebUI state (exit 0 ok, 3 stopped, 4 not installed)"
  Write-Host "  port --port=N     Show or set WebUI port (saved; restarts if running)"
  Write-Host "  webui run|stop    Start/stop the WebUI preview on its port"
  Write-Host "  webui [--url U]   Open the Web UI in the browser"
  Write-Host "  reinstall [--remove-data=yes|no] [-y]   Rebuild service (elevated)"
  Write-Host "  update            Rebuild API+CLI+WebUI, restart service if running"
  Write-Host "  remove [--purge] [-y]   Stop + uninstall (purge also deletes data/state)"
  Write-Host ""
  Write-Host "Service:"
  Write-Host "  service start|stop|restart|status"
  Write-Host "  install           Install the Windows service"
  Write-Host "  uninstall         Uninstall the Windows service"
  Write-Host "  api run           Run API in foreground (dev/console mode)"
  Write-Host ""
  Write-Host "App:"
  Write-Host "  doctor [--fix]    Diagnostics; --fix repairs stale pid, config, state dir"
  Write-Host "  version           Print CLI, service and WebUI build versions"
  Write-Host "  help              Show this help message"
  Write-Host ""
  Write-Host "Stats (one-shot, add --json for automation):"
  Write-Host "  overview | cpu | memory | disk | network | os | machine info"
  Write-Host "Live (streaming, Ctrl+C stops):"
  Write-Host "  cpu live | memory live | disk live | network live | live | tui"
}

# ---- dispatch ----------------------------------------------------------------

if ([string]::IsNullOrWhiteSpace($Command) -or $Command -in 'help', '--help', '-h') { Show-Help; exit 0 }
if ($Command -in 'version', '--version', '-v') { Invoke-Version; exit 0 }

function Invoke-Api {
  param([string[]]$ApiArgs)
  $apiExe = Get-AlamutApiExe
  if (-not $apiExe) {
    Write-Host "Error: alamut-api.exe not found. cd netvan-api; cargo build --release"
    exit 1
  }
  & $apiExe @ApiArgs
  exit $LASTEXITCODE
}

function Invoke-Cli {
  param([string[]]$CliArgs)
  $cliExe = Get-AlamutCliExe
  if (-not $cliExe) {
    Write-Host "Error: alamut-cli.exe not found. cd netvan-api; cargo build --release -p alamut-cli"
    exit 1
  }
  $globals = @()
  $rest = @()
  for ($i = 0; $i -lt $CliArgs.Count; $i++) {
    $a = $CliArgs[$i]
    if ($a -eq '--plain') { $globals += $a }
    elseif (($a -eq '--api' -or $a -eq '--poll-ms') -and ($i + 1) -lt $CliArgs.Count) {
      $globals += $a; $i++; $globals += $CliArgs[$i]
    }
    elseif ($a -like '--api=*' -or $a -like '--poll-ms=*') { $globals += $a }
    else { $rest += $a }
  }
  & $cliExe @($globals + $rest)
  exit $LASTEXITCODE
}

switch ($Command) {
  'run'       { exit (Invoke-Run) }
  'stop'      { exit (Invoke-Stop) }
  'status'    { exit (Invoke-Status) }
  'port'      { exit (Invoke-Port $Args) }
  'webui'     { Invoke-WebUI -Extra $Args }
  'reinstall' { exit (Invoke-Reinstall $Args) }
  'update'    { Invoke-Update }
  'rebuild'   { Invoke-Update }
  'remove'    { Invoke-Remove -Extra $Args }
  'doctor'    { exit (Invoke-Doctor $Args) }
  'install'   { Invoke-Api @('install') }
  'uninstall' { Invoke-Api @('uninstall') }
  'api'       {
    if ($Args.Count -gt 0 -and $Args[0] -eq 'run') { Invoke-Api (@('run') + $Args[1..($Args.Count - 1)]) }
    Write-Host "Usage: alamut api run"; exit 2
  }
  'service'   {
    if (-not $Args -or $Args.Count -eq 0) { Write-Host "Error: service requires start|stop|restart|status"; exit 2 }
    switch ($Args[0]) {
      'start'   { Invoke-Api @('start') }
      'stop'    { Invoke-Api @('stop') }
      'restart' {
        $apiExe = Get-AlamutApiExe
        & $apiExe stop; Start-Sleep -Seconds 2; & $apiExe start
        exit $LASTEXITCODE
      }
      'status'  { Invoke-Api @('status') }
      default   { Write-Host "Error: Unknown service action '$($Args[0])'"; exit 2 }
    }
  }
  'overview'  { Invoke-Cli (@('overview') + $Args) }
  'cpu'       { Invoke-Cli (@('cpu') + $Args) }
  'memory'    { Invoke-Cli (@('memory') + $Args) }
  'disk'      { Invoke-Cli (@('disk') + $Args) }
  'network'   { Invoke-Cli (@('network') + $Args) }
  'os'        { Invoke-Cli (@('os') + $Args) }
  'machine'   { Invoke-Cli (@('machine-info') + $Args) }
  'live'      { Invoke-Cli (@('live') + $Args) }
  'tui'       { Invoke-Cli @('tui') }
  default {
    Write-Host "Error: Unknown command '$Command'"
    Show-Help
    exit 2
  }
}
