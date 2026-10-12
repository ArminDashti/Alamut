//! `alamut-cli` — interactive geek-lovely TUI + headless CLI commands.
//!
//! TUI (default, no subcommand): full-screen colorful dashboard.
//! Headless: `overview | cpu | memory | disk | network | os |
//! machine-info | live` — script-friendly, `--json` for automation,
//! `<resource> live` for streaming views. Works despite the GUI.

mod api;
mod cmds;
mod state;
mod theme;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "alamut-cli",
    version,
    about = "Alamut CLI — geek-lovely terminal dashboard + headless commands",
    long_about = "Interactive TUI (default) plus headless commands that work despite the GUI: \
                  overview, cpu, memory, disk, network, os, machine-info and live views. \
                  Talks JSON-RPC to a running Alamut API (alamut run / alamut service start)."
)]
struct Args {
    /// API base URL (overrides ALAMUT_API_URL)
    #[arg(long, env = "ALAMUT_API_URL", global = true)]
    api: Option<String>,

    /// Snapshot poll interval in milliseconds (TUI + live views)
    #[arg(long, default_value_t = 1500, global = true)]
    poll_ms: u64,

    /// Disable colors / glyphs (also honors NO_COLOR)
    #[arg(long, global = true)]
    plain: bool,

    /// Fetch one overview snapshot as JSON and exit (headless connectivity check)
    #[arg(long, hide = true)]
    dump: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Interactive colorful terminal dashboard (default)
    Tui,
    /// One-shot overview: service + cpu + memory + disks + NICs
    Overview {
        /// Machine-readable JSON output
        #[arg(long)]
        json: bool,
    },
    /// CPU snapshot. `cpu live` streams.
    Cpu {
        /// Stream (`alamut cpu live`)
        #[arg(default_value = None)]
        live_word: Option<String>,
        /// Stream
        #[arg(long)]
        live: bool,
        /// Live refresh interval in ms
        #[arg(long, default_value_t = 1000)]
        interval_ms: u64,
        /// Machine-readable JSON output
        #[arg(long)]
        json: bool,
    },
    /// Memory snapshot. `memory live` streams.
    Memory {
        #[arg(default_value = None)]
        live_word: Option<String>,
        #[arg(long)]
        live: bool,
        #[arg(long, default_value_t = 1000)]
        interval_ms: u64,
        #[arg(long)]
        json: bool,
    },
    /// Disk volumes. `disk live` streams.
    Disk {
        #[arg(default_value = None)]
        live_word: Option<String>,
        #[arg(long)]
        live: bool,
        #[arg(long, default_value_t = 2000)]
        interval_ms: u64,
        #[arg(long)]
        json: bool,
    },
    /// NIC table. `network live` streams.
    Network {
        #[arg(default_value = None)]
        live_word: Option<String>,
        #[arg(long)]
        live: bool,
        #[arg(long, default_value_t = 1000)]
        interval_ms: u64,
        #[arg(long)]
        json: bool,
    },
    /// OS / host / API reachability
    Os {
        #[arg(long)]
        json: bool,
    },
    /// Static hardware inventory (`machine info`)
    #[command(name = "machine-info", alias = "machine")]
    MachineInfo {
        /// Accepts the trailing `info` in `alamut machine info`
        #[arg(default_value = None)]
        extra: Option<String>,
        /// Machine-readable JSON output
        #[arg(long)]
        json: bool,
    },
    /// Full live view: cpu + memory + network (Ctrl+C stops)
    Live {
        #[arg(long, default_value_t = 1000)]
        interval_ms: u64,
    },
    /// Print version
    Version,
}

fn is_live(word: &Option<String>, flag: bool) -> bool {
    flag || matches!(word.as_deref(), Some(w) if w.eq_ignore_ascii_case("live"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    cmds::set_plain(args.plain);
    let base = cmds::resolve_base(args.api.clone());

    if let Some(cmd) = args.command {
        let api = api::ApiClient::new(base.clone());
        match cmd {
            Command::Tui => ui::run(base, args.poll_ms).await,
            Command::Overview { json } => cmds::cmd_overview(&api, json).await,
            Command::Cpu { live_word, live, interval_ms, json } => {
                if is_live(&live_word, live) {
                    cmds::cmd_cpu_live(api, interval_ms).await
                } else {
                    cmds::cmd_cpu(&api, json).await
                }
            }
            Command::Memory { live_word, live, interval_ms, json } => {
                if is_live(&live_word, live) {
                    cmds::cmd_memory_live(api, interval_ms).await
                } else {
                    cmds::cmd_memory(&api, json).await
                }
            }
            Command::Disk { live_word, live, interval_ms, json } => {
                if is_live(&live_word, live) {
                    cmds::cmd_disk_live(api, interval_ms).await
                } else {
                    cmds::cmd_disk(&api, json).await
                }
            }
            Command::Network { live_word, live, interval_ms, json } => {
                if is_live(&live_word, live) {
                    cmds::cmd_network_live(api, interval_ms).await
                } else {
                    cmds::cmd_network(&api, json).await
                }
            }
            Command::Os { json } => cmds::cmd_os(&api, json).await,
            Command::MachineInfo { json, .. } => cmds::cmd_machine(&api, json).await,
            Command::Live { interval_ms } => cmds::cmd_live(api, interval_ms).await,
            Command::Version => {
                println!("alamut-cli {}", env!("CARGO_PKG_VERSION"));
                Ok(())
            }
        }
    } else {
        if args.dump {
            let api = api::ApiClient::new(base.clone());
            let snap = state::fetch_overview(&api).await;
            println!("{}", serde_json::to_string_pretty(&snap)?);
            if snap.connected {
                return Ok(());
            }
            return Err(anyhow::anyhow!("no response from API at {base}"));
        }
        ui::run(base, args.poll_ms).await
    }
}
