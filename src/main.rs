use std::ffi::OsString;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Path of Exile overlay for Wayland")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the game with the overlay alongside it. Steam launch option:
    /// `poe-overlay launch -- %command%`
    Launch {
        /// The game command line.
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        game: Vec<OsString>,
    },
    /// Runs the overlay for a game process (started by `launch`).
    #[command(hide = true)]
    Overlay {
        #[arg(long)]
        game_pid: i32,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Commands::Launch { game } => poe_overlay::launch::run(&game),
        Commands::Overlay { game_pid } => poe_overlay::overlay::run(game_pid),
    }
}
