#Requires -Version 5.1
<#
.SYNOPSIS
  Install or update Alamut as one local Windows service (WinSW): API + WebUI.

.DESCRIPTION
  Fresh install: assign ports (random safe API + WebUI=API+1, or "port set N"),
  ensure data dir, build API + WebUI, generate Start-Alamut.ps1 / Serve-WebUi.ps1 /
  Alamut.xml, install and start a single service named Alamut.

  Already installed: UPDATE path — fully remove prior app/service wrappers, rebuild,
  rewrite helpers/XML, install+start. Keeps %ProgramData%\Alamut\AlamutApi (SQLite).
  Reuses ports from state.json unless "port set N" overrides. Never drops the database.

.EXAMPLE
  .\install.ps1
  .\install.ps1 port set 18000

.NOTES
  Requires Administrator. Invoke manually.
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

if ([string]::IsNullOrWhiteSpace($ProjectRoot)) {
  $ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
}

$DeployDir = $PSScriptRoot
$StatePath = Join-Path $DeployDir 'state.json'
$XmlPath = Join-Path $DeployDir 'Alamut.xml'
$LegacyXmlPath = Join-Path $DeployDir 'alamut-api.xml'
$WinswDownloadPath = Join-Path $DeployDir 'winsw.exe'
$WinswPath = Join-Path $DeployDir "$ServiceName.exe"
$StartScript = Join-Path $DeployDir 'Start-Alamut.ps1'
$ServeScript = Join-Path $DeployDir 'Serve-WebUi.ps1'
$ApiRoot = Join-Path $ProjectRoot 'alamut-api'
$WebUiRoot = Join-Path $ProjectRoot 'alamut-webui'
$WebUiDist = Join-Path $WebUiRoot 'dist'
$DataDir = Join-Path $env:ProgramData 'Alamut\AlamutApi'
$LegacyServiceNames = @('alamut-api', 'alamut-webui', 'Alamut')
$PortMin = 20000
$PortMax = 49000

function Test-IsElevated {
  $id = [Security.Principal.WindowsIdentity]::GetCurrent()
  $p = New-Object Security.Principal.WindowsPrincipal($id)
  return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Read-State {
  if (-not (Test-Path -LiteralPath $StatePath)) { return $null }
  return (Get-Content -Raw -LiteralPath $StatePath | ConvertFrom-Json)
}

function Write-State([hashtable]$Values) {
  $Values | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $StatePath -Encoding UTF8
}

function Test-AlreadyInstalled {
  foreach ($name in $LegacyServiceNames) {
    $svc = Get-Service -Name $name -ErrorAction SilentlyContinue
    if ($svc) { return $true }
  }

  $st = Read-State
  if ($st -and $st.installed -eq $true) { return $true }

  if ((Test-Path -LiteralPath $WinswPath) -and (Test-Path -LiteralPath $XmlPath)) {
    try {
      $prev = $ErrorActionPreference
      $ErrorActionPreference = 'Continue'
      $out = & $WinswPath status 2>&1 | Out-String
      if ($out -match 'Installed|Running|Stopped') { return $true }
    } catch {
      # Corrupt/truncated WinSW must not block install; Ensure-Winsw redownloads.
    } finally {
      $ErrorActionPreference = $prev
    }
  }

  return $false
}

function Test-PortFree([int]$Port) {
  if ($Port -lt 1 -or $Port -gt 65535) { return $false }
  try {
    Import-Module NetTCPIP -ErrorAction SilentlyContinue | Out-Null
    $inUse = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
    if ($inUse) { return $false }
  } catch {
    # Fall through to TcpListener probe if NetTCPIP is unavailable
  }
  try {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $Port)
    $listener.Start()
    $listener.Stop()
    return $true
  } catch {
    return $false
  }
}

function Test-PortPairFree([int]$ApiPort) {
  $web = $ApiPort + 1
  if ($web -gt 65535) { return $false }
  return (Test-PortFree $ApiPort) -and (Test-PortFree $web)
}

function Get-PortSetFromArgs([string[]]$ArgsList) {
  if (-not $ArgsList -or $ArgsList.Count -eq 0) { return $null }
  if ($ArgsList.Count -lt 3) {
    throw 'Usage: .\install.ps1 [port set <NUMBER>]'
  }
  if ($ArgsList[0] -ne 'port' -or $ArgsList[1] -ne 'set') {
    throw "Unknown arguments: $($ArgsList -join ' '). Usage: .\install.ps1 [port set <NUMBER>]"
  }
  $raw = $ArgsList[2]
  $n = 0
  if (-not [int]::TryParse($raw, [ref]$n)) {
    throw "port set requires an integer; got '$raw'"
  }
  if ($n -lt 1 -or $n -gt 65534) {
    throw "API port must be 1..65534 (WebUI uses port+1); got $n"
  }
  if ($ArgsList.Count -gt 3) {
    throw "Unexpected extra arguments after port set: $($ArgsList[3..($ArgsList.Count-1)] -join ' ')"
  }
  return $n
}

function Get-RandomFreeApiPort {
  $rng = [System.Random]::new()
  for ($i = 0; $i -lt 200; $i++) {
    $candidate = $rng.Next($PortMin, $PortMax + 1)
    if (($candidate + 1) -gt 65535) { continue }
    if (Test-PortPairFree $candidate) { return $candidate }
  }
  throw "Could not find a free API/WebUI port pair in $PortMin-$PortMax"
}

function Get-NativeApiExe {
  $candidates = @(
    (Join-Path $ApiRoot 'target\release\alamut-api.exe'),
    (Join-Path $ApiRoot 'target\x86_64-pc-windows-gnu\release\alamut-api.exe'),
    (Join-Path $ApiRoot 'target\x86_64-pc-windows-msvc\release\alamut-api.exe')
  )
  foreach ($c in $candidates) {
    if (Test-Path -LiteralPath $c) { return (Resolve-Path -LiteralPath $c).Path }
  }
  return $null
}

function Invoke-WinswSafe([string]$Exe, [string[]]$WinArgs) {
  if (-not $Exe -or -not (Test-Path -LiteralPath $Exe)) { return }
  $base = [IO.Path]::GetFileNameWithoutExtension($Exe)
  $cfg = Join-Path (Split-Path -Parent $Exe) "$base.xml"
  if (-not (Test-Path -LiteralPath $cfg)) { return }
  try {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $Exe @WinArgs 2>&1 | Out-Null
  } catch {
  } finally {
    $ErrorActionPreference = $prev
  }
}

function Stop-NativeApiService {
  $exe = Get-NativeApiExe
  if (-not $exe) { return }
  try {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $exe stop 2>&1 | Out-Null
    & $exe uninstall 2>&1 | Out-Null
  } catch {
  } finally {
    $ErrorActionPreference = $prev
  }
}

function Remove-ScService([string]$Name) {
  $svc = Get-Service -Name $Name -ErrorAction SilentlyContinue
  if (-not $svc) { return }
  Write-Host "==> removing legacy/sc service $Name"
  sc.exe stop $Name 2>$null | Out-Null
  Start-Sleep -Milliseconds 500
  sc.exe delete $Name 2>$null | Out-Null
}

function Clear-PriorServices([int]$ApiPort, [int]$WebUiPort) {
  Write-Host '==> removing prior app/service (keeping ProgramData / SQLite)'
  Invoke-WinswSafe -Exe $WinswPath -WinArgs @('stop')
  Invoke-WinswSafe -Exe $WinswPath -WinArgs @('uninstall')
  $legacyExe = Join-Path $DeployDir 'winsw.exe'
  if (Test-Path -LiteralPath $legacyExe) {
    Invoke-WinswSafe -Exe $legacyExe -WinArgs @('stop')
    Invoke-WinswSafe -Exe $legacyExe -WinArgs @('uninstall')
  }
  $legacyApiExe = Join-Path $DeployDir 'alamut-api.exe'
  if (Test-Path -LiteralPath $legacyApiExe) {
    Invoke-WinswSafe -Exe $legacyApiExe -WinArgs @('stop')
    Invoke-WinswSafe -Exe $legacyApiExe -WinArgs @('uninstall')
  }

  foreach ($extra in @(
      (Join-Path $env:ProgramFiles 'Alamut\alamut-api.exe'),
      (Join-Path $env:ProgramFiles 'Alamut\alamut-webui.exe'),
      (Join-Path $env:ProgramFiles 'Alamut\Alamut.exe')
    )) {
    Invoke-WinswSafe -Exe $extra -WinArgs @('stop')
    Invoke-WinswSafe -Exe $extra -WinArgs @('uninstall')
  }

  Stop-NativeApiService

  foreach ($name in $LegacyServiceNames) {
    Remove-ScService $name
  }

  foreach ($port in @($ApiPort, $WebUiPort)) {
    try {
      Import-Module NetTCPIP -ErrorAction SilentlyContinue | Out-Null
      Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue |
        ForEach-Object {
          Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue
        }
    } catch {
      # Best-effort; TcpListener checks still gate installs
    }
  }

  # Strip prior generated app artifacts; never touch $DataDir.
  foreach ($gen in @(
      $WinswPath,
      $XmlPath,
      $LegacyXmlPath,
      $StartScript,
      $ServeScript,
      (Join-Path $DeployDir 'alamut-api.exe')
    )) {
    if (Test-Path -LiteralPath $gen) {
      Write-Host "==> removing prior app file $([IO.Path]::GetFileName($gen))"
      Remove-Item -LiteralPath $gen -Force -ErrorAction SilentlyContinue
    }
  }
}

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

function Build-Api {
  if (-not (Test-Path -LiteralPath $ApiRoot)) { throw "Missing API root: $ApiRoot" }
  if ($env:ALAMUT_INSTALL_SKIP_API_BUILD -eq '1') {
    $existing = Get-NativeApiExe
    if (-not $existing) {
      throw 'ALAMUT_INSTALL_SKIP_API_BUILD=1 but alamut-api.exe not found under target/*/release'
    }
    Write-Host "==> skipping cargo build (ALAMUT_INSTALL_SKIP_API_BUILD=1); using $existing"
    return $existing
  }
  $target = Resolve-ApiCargoTarget
  Push-Location $ApiRoot
  try {
    Write-Host "==> cargo build -p alamut-api --release --target $target"
    # Out-Host keeps native output off the pipeline so the function returns only the path.
    cargo build -p alamut-api --release --target $target | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }
  } finally {
    Pop-Location
  }
  $bin = Get-NativeApiExe
  if (-not $bin) { throw 'alamut-api.exe not found under target/*/release' }
  return $bin
}

function Build-WebUi {
  if (-not (Test-Path -LiteralPath $WebUiRoot)) {
    throw "Missing WebUI root: $WebUiRoot"
  }
  if ($env:ALAMUT_INSTALL_SKIP_WEBUI_BUILD -eq '1') {
    if (-not (Test-Path -LiteralPath $WebUiDist)) {
      throw "ALAMUT_INSTALL_SKIP_WEBUI_BUILD=1 but dist missing: $WebUiDist"
    }
    Write-Host "==> skipping npm run build (ALAMUT_INSTALL_SKIP_WEBUI_BUILD=1); using $WebUiDist"
    return (Resolve-Path -LiteralPath $WebUiDist).Path
  }
  Push-Location $WebUiRoot
  try {
    $packageJson = Join-Path $WebUiRoot 'package.json'
    $packageLock = Join-Path $WebUiRoot 'package-lock.json'
    if (-not (Test-Path -LiteralPath $packageJson)) {
      throw "WebUI package manifest missing: $packageJson"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $WebUiRoot 'node_modules'))) {
      if (Test-Path -LiteralPath $packageLock) {
        Write-Host '==> npm ci (webui dependencies)'
        npm ci | Out-Host
      } else {
        Write-Host '==> npm install (webui dependencies)'
        npm install | Out-Host
      }
      if ($LASTEXITCODE -ne 0) {
        throw "WebUI dependency installation failed ($LASTEXITCODE)"
      }
    }
    Write-Host '==> npm run build (webui)'
    npm run build | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "npm run build failed ($LASTEXITCODE)" }
  } finally {
    Pop-Location
  }
  if (-not (Test-Path -LiteralPath $WebUiDist)) {
    throw "WebUI dist missing after build: $WebUiDist"
  }
  return (Resolve-Path -LiteralPath $WebUiDist).Path
}

function Test-WinswBinary([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path)) { return $false }
  $item = Get-Item -LiteralPath $Path
  # WinSW v2.12.0 WinSW.NET461.exe official asset size (truncated downloads break install).
  $expectedSize = 655872L
  if ($item.Length -ne $expectedSize) { return $false }
  try {
    $fs = [IO.File]::OpenRead($Path)
    try {
      $hdr = New-Object byte[] 2
      if ($fs.Read($hdr, 0, 2) -ne 2) { return $false }
      if ($hdr[0] -ne 0x4D -or $hdr[1] -ne 0x5A) { return $false }
    } finally { $fs.Close() }
  } catch { return $false }
  return $true
}

function Ensure-Winsw {
  # Prefer framework-dependent build: ~640KB vs ~17MB self-contained WinSW-x64.exe.
  $url = 'https://github.com/winsw/winsw/releases/download/v2.12.0/WinSW.NET461.exe'
  $expectedSize = 655872L
  $needDownload = -not (Test-WinswBinary $WinswDownloadPath)
  if ($needDownload -and (Test-Path -LiteralPath $WinswDownloadPath)) {
    Write-Host "==> removing invalid/truncated WinSW ($((Get-Item -LiteralPath $WinswDownloadPath).Length) bytes; need $expectedSize)"
    Remove-Item -LiteralPath $WinswDownloadPath -Force
  }
  if ($needDownload) {
    Write-Host "==> downloading WinSW to $WinswDownloadPath"
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $url -OutFile $WinswDownloadPath -UseBasicParsing
    if (-not (Test-WinswBinary $WinswDownloadPath)) {
      $got = if (Test-Path -LiteralPath $WinswDownloadPath) { (Get-Item -LiteralPath $WinswDownloadPath).Length } else { 0 }
      Remove-Item -LiteralPath $WinswDownloadPath -Force -ErrorAction SilentlyContinue
      throw "WinSW download invalid (got $got bytes, need $expectedSize MZ PE). Tried: $url"
    }
  }
  # WinSW v2 resolves config as <exe-basename>.xml next to the executable.
  Copy-Item -LiteralPath $WinswDownloadPath -Destination $WinswPath -Force
  Unblock-File -LiteralPath $WinswPath -ErrorAction SilentlyContinue
  Write-Host "==> WinSW service wrapper ready: $WinswPath"
}

function Write-GeneratedHelpers {
  $serveBody = @'
#Requires -Version 5.1
<#
.SYNOPSIS
  Serve a static WebUI folder over HTTP (no Node required at runtime).
  Generated by install.ps1 — do not edit as source of truth.
#>
param(
  [Parameter(Mandatory = $true)]
  [string]$Root,

  [int]$Port = 8001,

  [string]$HostAddress = '127.0.0.1'
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $Root)) {
  throw "WebUI root not found: $Root"
}

$Root = (Resolve-Path -LiteralPath $Root).Path
$prefix = "http://${HostAddress}:$Port/"

$mime = @{
  '.html' = 'text/html; charset=utf-8'
  '.htm'  = 'text/html; charset=utf-8'
  '.js'   = 'application/javascript; charset=utf-8'
  '.mjs'  = 'application/javascript; charset=utf-8'
  '.css'  = 'text/css; charset=utf-8'
  '.json' = 'application/json; charset=utf-8'
  '.svg'  = 'image/svg+xml'
  '.png'  = 'image/png'
  '.jpg'  = 'image/jpeg'
  '.jpeg' = 'image/jpeg'
  '.webp' = 'image/webp'
  '.ico'  = 'image/x-icon'
  '.woff' = 'font/woff'
  '.woff2'= 'font/woff2'
  '.map'  = 'application/json'
  '.webmanifest' = 'application/manifest+json'
  '.txt'  = 'text/plain; charset=utf-8'
}

$listener = [System.Net.HttpListener]::new()
$listener.Prefixes.Add($prefix)
$listener.Start()
Write-Host "Serving $Root at $prefix"

try {
  while ($listener.IsListening) {
    $ctx = $listener.GetContext()
    $req = $ctx.Request
    $res = $ctx.Response
    try {
      $rel = [Uri]::UnescapeDataString($req.Url.AbsolutePath.TrimStart('/'))
      if ([string]::IsNullOrWhiteSpace($rel)) { $rel = 'index.html' }
      $rel = $rel -replace '/', [IO.Path]::DirectorySeparatorChar
      $candidate = [IO.Path]::GetFullPath((Join-Path $Root $rel))
      if (-not $candidate.StartsWith($Root, [StringComparison]::OrdinalIgnoreCase)) {
        $res.StatusCode = 403
        $res.Close()
        continue
      }
      if ((Test-Path -LiteralPath $candidate -PathType Container)) {
        $candidate = Join-Path $candidate 'index.html'
      }
      if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
        $candidate = Join-Path $Root 'index.html'
      }
      if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
        $res.StatusCode = 404
        $res.Close()
        continue
      }
      $ext = [IO.Path]::GetExtension($candidate).ToLowerInvariant()
      $res.ContentType = if ($mime.ContainsKey($ext)) { $mime[$ext] } else { 'application/octet-stream' }
      $bytes = [IO.File]::ReadAllBytes($candidate)
      $res.ContentLength64 = $bytes.LongLength
      $res.OutputStream.Write($bytes, 0, $bytes.Length)
      $res.StatusCode = 200
    } catch {
      $res.StatusCode = 500
    } finally {
      $res.Close()
    }
  }
} finally {
  if ($listener.IsListening) { $listener.Stop() }
  $listener.Close()
}
'@

  $startBody = @'
#Requires -Version 5.1
<#
.SYNOPSIS
  WinSW entrypoint: run API + WebUI under one process tree (service Alamut).
  Generated by install.ps1 — do not edit as source of truth.
#>
param(
  [Parameter(Mandatory = $true)]
  [string]$ApiExe,

  [Parameter(Mandatory = $true)]
  [string]$ApiWorkingDir,

  [Parameter(Mandatory = $true)]
  [string]$WebUiRoot,

  [Parameter(Mandatory = $true)]
  [string]$ServeScript,

  [string]$DataDir = (Join-Path $env:ProgramData 'Alamut\AlamutApi'),

  [int]$ApiPort = 8000,

  [int]$WebUiPort = 8001,

  [string]$BindHost = '127.0.0.1'
)

$ErrorActionPreference = 'Stop'

foreach ($need in @($ApiExe, $ApiWorkingDir, $WebUiRoot, $ServeScript)) {
  if (-not (Test-Path -LiteralPath $need)) {
    throw "Missing path required by Start-Alamut: $need"
  }
}

$env:ALAMUT_API_BIND = "${BindHost}:$ApiPort"
$env:ALAMUT_API_DATA_DIR = $DataDir

$ps = (Get-Command powershell.exe).Source
$webArgs = @(
  '-NoProfile',
  '-ExecutionPolicy', 'Bypass',
  '-File', $ServeScript,
  '-Root', $WebUiRoot,
  '-Port', "$WebUiPort",
  '-HostAddress', $BindHost
)

Write-Host "==> starting API $ApiExe run (bind $($env:ALAMUT_API_BIND))"
$api = Start-Process -FilePath $ApiExe -ArgumentList @('run') `
  -WorkingDirectory $ApiWorkingDir -PassThru -WindowStyle Hidden

Write-Host "==> starting WebUI static server on ${BindHost}:$WebUiPort"
$web = Start-Process -FilePath $ps -ArgumentList $webArgs `
  -WorkingDirectory (Split-Path -Parent $ServeScript) -PassThru -WindowStyle Hidden

function Stop-Child([System.Diagnostics.Process]$Proc) {
  if (-not $Proc) { return }
  if ($Proc.HasExited) { return }
  try {
    Stop-Process -Id $Proc.Id -Force -ErrorAction SilentlyContinue
  } catch {}
  try {
    & taskkill.exe /F /T /PID $Proc.Id 2>$null | Out-Null
  } catch {}
}

try {
  while ($true) {
    Start-Sleep -Seconds 2
    if ($api.HasExited) {
      Write-Host "==> API exited with code $($api.ExitCode)"
      break
    }
    if ($web.HasExited) {
      Write-Host "==> WebUI exited with code $($web.ExitCode)"
      break
    }
  }
} finally {
  Write-Host '==> stopping API + WebUI children'
  Stop-Child $web
  Stop-Child $api
}
'@

  Set-Content -LiteralPath $ServeScript -Value $serveBody -Encoding UTF8
  Set-Content -LiteralPath $StartScript -Value $startBody -Encoding UTF8
  Write-Host "==> wrote generated helpers: $StartScript , $ServeScript"
}

function Write-ServiceXml([string]$BinaryPath, [string]$DistPath, [int]$ApiPort, [int]$WebUiPort) {
  $psEsc = [System.Security.SecurityElement]::Escape((Get-Command powershell.exe).Source)
  $dataEsc = [System.Security.SecurityElement]::Escape($DataDir)
  $wdEsc = [System.Security.SecurityElement]::Escape($DeployDir)
  $bind = "127.0.0.1:$ApiPort"
  $args = "-NoProfile -ExecutionPolicy Bypass -File `"$StartScript`" -ApiExe `"$BinaryPath`" -ApiWorkingDir `"$ApiRoot`" -WebUiRoot `"$DistPath`" -ServeScript `"$ServeScript`" -DataDir `"$DataDir`" -ApiPort $ApiPort -WebUiPort $WebUiPort -BindHost 127.0.0.1"
  $argsEsc = [System.Security.SecurityElement]::Escape($args)
  $bindEsc = [System.Security.SecurityElement]::Escape($bind)

  $xml = @"
<?xml version="1.0" encoding="UTF-8"?>
<service>
  <id>$ServiceName</id>
  <name>$ServiceName</name>
  <description>$StackName API + WebUI (local Windows)</description>
  <executable>$psEsc</executable>
  <arguments>$argsEsc</arguments>
  <workingdirectory>$wdEsc</workingdirectory>
  <env name="ALAMUT_API_BIND" value="$bindEsc"/>
  <env name="ALAMUT_API_DATA_DIR" value="$dataEsc"/>
  <logpath>$dataEsc</logpath>
  <log mode="roll-by-size">
    <sizeThreshold>10240</sizeThreshold>
    <keepFiles>8</keepFiles>
  </log>
  <onfailure action="restart" delay="10 sec"/>
  <stoptimeout>20 sec</stoptimeout>
</service>
"@
  $utf8NoBom = New-Object System.Text.UTF8Encoding $false
  [System.IO.File]::WriteAllText($XmlPath, $xml, $utf8NoBom)
  Write-Host "==> wrote $XmlPath"
}

function Invoke-Winsw([string[]]$WinArgs) {
  # Config is Alamut.xml beside Alamut.exe (same basename) — do not pass XML path.
  & $WinswPath @WinArgs
  if ($LASTEXITCODE -ne 0) {
    throw "winsw $($WinArgs -join ' ') failed ($LASTEXITCODE)"
  }
}

function Ensure-AlamutOnPath {
  # alamut.bat/alamut.ps1 live in the project root; put it on the user PATH once.
  $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  $entries = @($userPath -split ';' | Where-Object { $_ })
  if ($entries -contains $ProjectRoot) {
    Write-Host "==> alamut already on user PATH ($ProjectRoot)"
    return
  }
  [Environment]::SetEnvironmentVariable('Path', (@($entries + $ProjectRoot) -join ';'), 'User')
  Write-Host "==> added $ProjectRoot to user PATH (open a new terminal to use 'alamut')"
}

function Ensure-DataDir {
  if (-not (Test-Path -LiteralPath $DataDir)) {
    New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
    Write-Host "==> created data dir $DataDir (schema applies on first API open; no DROP)"
  } else {
    Write-Host "==> keeping existing data dir $DataDir"
  }
}

# --- main ---
if (-not (Test-IsElevated)) {
  throw 'Administrator elevation required to install/update the Windows service. Re-run from an elevated PowerShell.'
}

if (-not (Test-Path -LiteralPath $ApiRoot)) {
  throw "Expected API at $ApiRoot"
}
if (-not (Test-Path -LiteralPath $WebUiRoot)) {
  throw "Expected WebUI at $WebUiRoot"
}

$portOverride = Get-PortSetFromArgs $CliArgs
$already = Test-AlreadyInstalled
if ($already) {
  Write-Host '==> UPDATE path (already installed); keeping DB/data'
} else {
  Write-Host '==> FRESH install path'
}

$prior = Read-State
if ($already -and $prior -and $prior.data_dir) {
  $DataDir = [string]$prior.data_dir
}

$ApiPort = 0
$WebUiPort = 0
if ($null -ne $portOverride) {
  $ApiPort = [int]$portOverride
  $WebUiPort = $ApiPort + 1
  Write-Host "==> port set: API=$ApiPort WebUI=$WebUiPort"
} elseif ($already -and $prior -and $prior.ports -and $prior.ports.api) {
  $ApiPort = [int]$prior.ports.api
  if ($prior.ports.webui) {
    $WebUiPort = [int]$prior.ports.webui
  } else {
    $WebUiPort = $ApiPort + 1
  }
  Write-Host "==> reusing ports from state: API=$ApiPort WebUI=$WebUiPort"
} else {
  $ApiPort = Get-RandomFreeApiPort
  $WebUiPort = $ApiPort + 1
  Write-Host "==> random safe ports: API=$ApiPort WebUI=$WebUiPort"
}

Ensure-DataDir
Clear-PriorServices -ApiPort $ApiPort -WebUiPort $WebUiPort

# After clearing listeners, re-check override / random ports
if ($null -ne $portOverride) {
  if (-not (Test-PortPairFree $ApiPort)) {
    throw "Ports $ApiPort / $WebUiPort are still in use after cleanup"
  }
} elseif (-not $already) {
  if (-not (Test-PortPairFree $ApiPort)) {
    $ApiPort = Get-RandomFreeApiPort
    $WebUiPort = $ApiPort + 1
    Write-Host "==> re-picked ports after cleanup: API=$ApiPort WebUI=$WebUiPort"
  }
}

$binary = Build-Api
$dist = Build-WebUi
Ensure-Winsw
Write-GeneratedHelpers
Write-ServiceXml -BinaryPath $binary -DistPath $dist -ApiPort $ApiPort -WebUiPort $WebUiPort

Invoke-WinswSafe -Exe $WinswPath -WinArgs @('stop')
Invoke-WinswSafe -Exe $WinswPath -WinArgs @('uninstall')

Invoke-Winsw @('install')
Invoke-Winsw @('start')

Write-State @{
  stack_name   = $StackName
  service_name = $ServiceName
  installed    = $true
  ports        = @{ api = $ApiPort; webui = $WebUiPort }
  binary_path  = $binary
  webui_dist   = $dist
  data_dir     = $DataDir
  mode         = $(if ($already) { 'update' } else { 'fresh' })
  updated_at   = (Get-Date).ToString('s')
}

Ensure-AlamutOnPath

Write-Host "Done ($($(if ($already) { 'update' } else { 'fresh' }))): single service '$ServiceName'"
Write-Host "  API   http://127.0.0.1:$ApiPort"
Write-Host "  WebUI http://127.0.0.1:$WebUiPort"
Write-Host "  data  $DataDir"
$global:LASTEXITCODE = 0
