use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::ipc;

#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// Use a macOS-style look
    #[arg(long)]
    pub macos: bool,
    /// Path to a theme JSON file or either "dark" or "light", see repo for example theme files
    #[arg(long, value_name = "PATH")]
    pub theme: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Open the switcher (or advance the selection if it is already open)
    Go {
        /// Pre-select going backwards
        #[arg(long)]
        reverse: bool,
    },
    /// Quit the daemon
    Quit,
}

impl Command {
    pub fn wire(self) -> &'static str {
        match self {
            Command::Go { reverse: false } => "go",
            Command::Go { reverse: true } => "go-reverse",
            Command::Quit => "quit",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "go" => Some(Command::Go { reverse: false }),
            "go-reverse" => Some(Command::Go { reverse: true }),
            "quit" => Some(Command::Quit),
            _ => None,
        }
    }
}

pub fn handle_client_command(cmd: Command) -> Result<()> {
    if ipc::send_command(cmd.wire()) {
        Ok(())
    } else {
        anyhow::bail!(
            "lwsfh daemon is not running. Start it (e.g. `exec-once = lwsfh` in your Hyprland config)."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_and_parse_roundtrip() {
        for cmd in [
            Command::Go { reverse: false },
            Command::Go { reverse: true },
            Command::Quit,
        ] {
            assert_eq!(Command::parse(cmd.wire()), Some(cmd));
        }
    }

    #[test]
    fn parse_rejects_unknown() {
        assert_eq!(Command::parse("wobble"), None);
        assert_eq!(Command::parse(""), None);
    }
}
