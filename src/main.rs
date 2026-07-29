mod cli;
mod daemon;
mod hyprland;
mod icon;
mod ipc;
mod overlay;
mod view;

use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        // A subcommand means we are acting as a thin client that pokes the
        // running daemon over its Unix socket.
        Some(cmd) => cli::handle_client_command(cmd),
        // No subcommand: become the long-running daemon that hosts the GPUI app.
        None => daemon::run(cli.macos, cli.theme),
    }
}
