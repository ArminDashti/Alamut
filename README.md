<p align="center">
  <img src="netvan-webui/public/icons/icon-512.png" alt="Alamut logo" width="128" />
</p>

# Alamut (netvan)

Alamut is a local Windows network and system monitor. It runs as a Windows service that collects metrics, keeps history in SQLite, and serves them to a web dashboard and a terminal UI. The API listens only on localhost. Only the network tools (ping, speedtest, IP lookup) contact external hosts.

- **Live dashboard** for CPU, memory, disks, network adapters, bandwidth, latency, thermals, and top apps by traffic
- **Network tools**: ping, HTTP latency, traceroute, nslookup, and speedtest, with live streamed output
- **IP lookup** for public IP details
- **Terminal CLI** (`alamut`) with a full-screen TUI and headless commands for scripts (`--json`)
- **Single service** that runs the API and the WebUI together, auto-starts, and restarts on failure

> Windows only. Alamut uses Windows APIs and the Windows Service Control Manager.

## Repository layout

| Path | What it is |
| --- | --- |
| `netvan-api/` | Rust workspace: the `alamut-api` service, the `alamut-cli` terminal app, and the collector and core crates |
| `netvan-webui/` | React + Vite PWA dashboard (display only; all work happens in the API) |
| `scripts/` | Install, start, serve, and code-signing PowerShell scripts |
| `docs/` | Extra documentation, including code signing |
| `alamut.ps1`, `alamut.bat` | `alamut` command wrappers placed on your PATH by the installer |
| `CLI.md` | Full CLI reference |

Architecture:

```text
Browser (netvan-webui) ──POST /api/rpc──────▶ alamut-api (127.0.0.1:<api port>)
                       ──WS  /api/ws/tools──▶   ├─ collectors (NIC, CPU, memory, disk,
alamut-cli (TUI)       ──POST /api/rpc──────▶   │   latency, speedtest, traceroute, ...)
                                                 └─ SQLite history (ProgramData)
```

## Requirements

- Windows 10 or later
- Administrator rights for installing the service
- [Rust](https://rustup.rs/) toolchain (edition 2021) to build the API
- Node.js 20+ to build the WebUI
- PowerShell 5.1+

## Quick start

1. Clone the repository:

   ```powershell
   git clone https://github.com/ArminDashti/netvan.git
   cd netvan
   ```

2. Open PowerShell **as Administrator** and run the installer:

   ```powershell
   .\scripts\install.ps1
   ```

   The installer builds the API and the WebUI, registers a single Windows service named `Alamut`, starts it, and adds the project folder to your user `PATH`. It prints the API and WebUI URLs when it finishes.

3. Open a new terminal and check the stack:

   ```powershell
   alamut status
   alamut doctor
   ```

4. Open the WebUI at the URL printed by the installer (the WebUI port is the API port + 1).

### Choosing ports

The installer picks a random free API/WebUI port pair in the 20000–49000 range on the first install and reuses it on updates. To choose the API port yourself:

```powershell
.\scripts\install.ps1 port set 18000   # API 18000, WebUI 18001
```

Both services bind to `127.0.0.1` only.

## Using the CLI

```powershell
alamut overview          # service + CPU + memory + disks + NICs
alamut network live      # stream NIC throughput
alamut tui               # full-screen terminal dashboard
alamut doctor --fix      # diagnostics and simple repairs
alamut help              # all commands
```

Common lifecycle commands:

| Command | Purpose |
| --- | --- |
| `alamut run` / `alamut stop` | Start or stop the API service and WebUI |
| `alamut status` | Service and WebUI state |
| `alamut update` | Rebuild and restart after pulling new code |
| `alamut remove [--purge]` | Uninstall (`--purge` also deletes the data directory) |

See [CLI.md](CLI.md) for every command and option.

### Terminal UI keys

`1`–`7` jump between tabs, `Tab` / `Shift+Tab` cycle, `r` refresh, `?` help, `q` quit.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `ALAMUT_API_BIND` | `127.0.0.1:28437` | API listen address |
| `ALAMUT_API_DATA_DIR` | `%ProgramData%\Alamut\AlamutApi\` | Data directory (SQLite database `alamut-web.db`) |
| `ALAMUT_API_URL` / `ALAMUT_API_BASE` | `http://127.0.0.1:28437` | Endpoint used by `alamut-cli` |
| `VITE_ALAMUT_API_URL` | same origin (dev proxy) | API origin for the WebUI build |

The installer stores the chosen install location and WebUI port in `%LOCALAPPDATA%\Alamut\config.json`. You can pick the install location (`%LOCALAPPDATA%\Alamut`, `%APPDATA%\Alamut`, `%ProgramFiles%\Alamut`, or a custom path) with `alamut install --location=...`.

## Development

Run the API in the foreground instead of as a service:

```powershell
cd netvan-api
cargo run -p alamut-api -- run
```

Run the WebUI with hot reload:

```powershell
cd netvan-webui
npm install
npm run dev        # http://127.0.0.1:28438, proxies /api to the API
```

Build the release binaries:

```powershell
cd netvan-api
cargo build --release -p alamut-cli
cargo build -p alamut-api --release

cd ..\netvan-webui
npm run build
```

If `cargo` links against MSVC instead of GNU, add `--target x86_64-pc-windows-msvc`.

### API endpoints

| Method | Path | Notes |
| --- | --- | --- |
| GET | `/api/health` | Liveness check |
| GET | `/api/data-dir` | Data directory path |
| POST | `/api/rpc` | JSON-RPC request and response |
| WS | `/api/ws/tools` | Live ping, traceroute, speedtest, and IP lookup streams |

Full details are in [netvan-api/README.md](netvan-api/README.md) and `netvan-api/docs/endpoints.md`.

## Code signing

Release binaries can be Authenticode-signed and hashed with `scripts/Sign-Binaries.ps1`, and verified with `scripts/Verify-Hash.ps1`. See [docs/code-signing.md](docs/code-signing.md).

## Troubleshooting

- **`alamut` not found** after install: open a new terminal so the updated user `PATH` is loaded.
- **Access denied / service changes fail**: run PowerShell as Administrator.
- **API unreachable** in the CLI or WebUI: run `alamut status`, then `alamut doctor`. Check that the service is running and that the port matches.
- **Stale WebUI PID**: `alamut doctor --fix` clears a leftover `webui.pid`.
- **Clean reinstall** without losing history: `alamut reinstall --remove-data=no`. Use `--remove-data=yes` only if you want to delete the database.

## Contributing

Issues and pull requests are welcome. Keep changes focused, and describe the behavior you changed. Run `cargo build` in `netvan-api` and `npm run build` in `netvan-webui` before opening a PR.

## License

Released under the [MIT License](https://opensource.org/licenses/MIT), as declared in the workspace `Cargo.toml` and `metadata.json`.

Author: Armin Dashti (Dashti Technologies LLC)
