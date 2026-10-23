# Alamut CLI

The `alamut` CLI provides a unified interface for managing the Alamut Windows service
plus an interactive, colorful terminal dashboard (`alamut tui`).

## Interactive TUI

`alamut-cli` is a full-screen, modern-colorful terminal UI (ratatui) that reads live
data from a running Alamut API over JSON-RPC (`POST /api/rpc`).

### Launch

```cmd
:: via the wrapper (finds target\release\alamut-cli.exe)
alamut tui

:: or directly
alamut-cli
alamut-cli --api http://127.0.0.1:28437
alamut-cli --poll-ms 1000
```

Environment overrides: `ALAMUT_API_URL` (or `ALAMUT_API_BASE`) for the endpoint,
`ALAMUT_API_BIND` as a fallback (rendered as `http://<bind>`).

Headless connectivity check (prints the overview snapshot as JSON, exit code 1 if
the API cannot be reached):

```cmd
alamut-cli --dump --api http://127.0.0.1:28437
```

Build it:

```powershell
cd alamut-api
cargo build --release -p alamut-cli
```

### Tabs

| # | Tab | Shows |
| --- | --- | --- |
| 1 | Overview | CPU / memory / disk gauges, adapter summary, throughput sparkline, service status |
| 2 | NICs | adapter table (link speed, IPv4, Wi-Fi, RSSI, live rx/tx) with a selectable NIC filter |
| 3 | Bandwidth | rx/tx line chart for today plus now / avg / peak rates |
| 4 | Latency | ping RTT + HTTP total charts and link up/down events |
| 5 | System | per-core CPU bars, memory, disk volumes, thermal sensors |
| 6 | Apps | top processes by traffic today |
| 7 | Tools | run ping, HTTP latency, traceroute, nslookup, speedtest and read the output |

### Keys

- `1`…`7` jump to a tab, `Tab` / `Shift+Tab` cycle, `r` refresh, `?` help, `q` quit
- NICs / Bandwidth / Latency: `↑`/`↓` move the cursor (row 1 = all NICs), `Enter` applies the filter
- Tools: `←`/`→` switch tool, `i`/`Enter` focus the target input, `r` run, `c` clear,
  `↑`/`↓` scroll output, `a` accept the Ookla speedtest EULA

### Behavior

- The active view polls the API every 1.5 s (configurable with `--poll-ms`);
  other tabs refresh when you switch to them.
- If the API is not reachable the UI stays up and shows an OFFLINE banner with
  recovery hints.
- Tool runs (ping / traceroute / speedtest …) execute on the API host and stream
  their result back into the output panel.

## Installation

### Local (Project Directory)
Use the CLI directly from the project root:
```powershell
powershell -ExecutionPolicy Bypass -File .\alamut.ps1 <command>
```

Or use the batch wrapper:
```cmd
alamut.bat <command>
```

### Global Installation
Run the installation script to add `alamut` to your PATH:

**For current user:**
```powershell
.\scripts\install-cli.ps1
```

**For system-wide installation (requires Administrator):**
```powershell
# Run PowerShell as Administrator
.\scripts\install-cli.ps1
```

After installation, restart your terminal and use:
```cmd
alamut <command>
```

## Commands

### Lifecycle (API service + WebUI)
- `alamut run` - Start the API service and the WebUI (`127.0.0.1:<port>`)
- `alamut stop` - Stop the WebUI and the API service
- `alamut status` - Service + WebUI state (exit 0 running, 3 stopped, 4 not installed)
- `alamut port [--port=N]` - Show or set the WebUI port (saved to `%LOCALAPPDATA%\Alamut\config.json`)
- `alamut webui run|stop` - Start or stop the WebUI preview (builds it if `dist` is missing)
- `alamut webui [--url <url>]` - Open the Web UI in the default browser
- `alamut reinstall [--remove-data=yes|no] [--location=...] [-y]` - Rebuild the service (elevated; `yes` deletes API data)
- `alamut remove [--purge] [-y]` - Stop + uninstall; scans for leftovers and exits 1 if any remain
- `alamut doctor [--fix]` - Diagnostics; `--fix` clears a stale `webui.pid` and repairs config
- `alamut api run` - Run the API in the foreground (the old `run` behavior)

### Service Management
- `alamut service start` - Start the Alamut Windows service
- `alamut service stop` - Stop the Alamut Windows service
- `alamut service restart` - Restart the Alamut Windows service
- `alamut service status` - Show service install/running status
- `alamut install [--location=...]` - Copy binaries to the install location and install the Alamut Windows service. Location is chosen from a menu when not given: `%LOCALAPPDATA%\Alamut` (default), `%APPDATA%\Alamut`, `%ProgramFiles%\Alamut`, or a custom path. Data stays in `%ProgramData%\Alamut\AlamutApi`.

### App
- `alamut doctor` - Run diagnostics (binaries, service, API health, data dir, WebUI, hosts entry)
- `alamut help` - Show help message
- `alamut version` - Print CLI + service binary versions
- `alamut remove [--purge]` - Stop + uninstall the service (`--purge` also deletes the data dir)
- `alamut update` - Rebuild API + CLI + WebUI, restart the service if it was running
- `alamut webui [--url <url>]` - Open the Web UI in the default browser

### Stats (headless, work despite the GUI; add `--json` for automation)
- `alamut overview` - Service + cpu + memory + disks + NICs
- `alamut cpu` - CPU snapshot (brand, load, per-core)
- `alamut memory` - Memory snapshot
- `alamut disk` - Disk volumes
- `alamut network` - NIC table (link, IPv4, live rx/tx)
- `alamut os` - OS / host / API reachability
- `alamut machine info` - Static hardware inventory (cpu/mem/disks/gpus/board)

### Live (streaming, Ctrl+C stops, `--interval-ms N`)
- `alamut cpu live` - Stream CPU load
- `alamut memory live` - Stream memory usage
- `alamut disk live` - Stream disk usage
- `alamut network live` - Stream NIC throughput
- `alamut live` - Full live view (cpu + memory + network)
- `alamut tui` - Interactive colorful terminal dashboard

Style: geek-lovely 24-bit color + nerd glyphs + block meters.
Disable with `--plain` or `NO_COLOR=1`. `cpu`/`memory`/`disk`/`os`
fall back to local collection when the API is down.

## Examples

```cmd
# Install the service
alamut install

# Check service status
alamut service status

# Start the service
alamut service start

# Run in foreground for development
alamut run

# Run diagnostics
alamut doctor

# Show help
alamut help
```

## How It Works

The `alamut` CLI is a wrapper that delegates to the `alamut-api` binary. It:
1. Locates the `alamut-api.exe` in the project's build directory
2. Delegates service commands to the native Windows service implementation
3. Provides additional convenience commands like `doctor` for diagnostics

## Requirements

- Windows 10 or later
- PowerShell 5.1 or later
- `alamut-api.exe` built from the `alamut-api` crate

## Building alamut-api

If `alamut-api.exe` is not found, build it:
```powershell
cd alamut-api
cargo build --release
```

The CLI will look for the binary in:
- `alamut-api/target/release/alamut-api.exe`
- `alamut-api/target/x86_64-pc-windows-gnu/release/alamut-api.exe`
- `alamut-api/target/x86_64-pc-windows-msvc/release/alamut-api.exe`
