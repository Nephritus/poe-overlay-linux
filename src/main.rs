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
    /// Sends an Action to the running Overlay; bind this to a key in your
    /// compositor, e.g. `poe-overlay trigger currency-rates`.
    Trigger {
        #[arg(value_enum)]
        action: poe_overlay::running::Action,
    },
    /// Puts things into your desktop's own config.
    Setup {
        #[command(subcommand)]
        what: Setup,
    },
    /// Shows credits and licenses for bundled third-party data.
    Credits,
}

#[derive(Subcommand)]
enum Setup {
    /// Binds the keys from [hotkeys] in config.toml in your compositor's
    /// config (niri); the game also asks to do this on its first start.
    Hotkeys {
        /// Take poe-overlay's binds out again, and stop the game asking.
        #[arg(long)]
        remove: bool,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Commands::Launch { game } => poe_overlay::launch::run(&game),
        Commands::Overlay { game_pid } => poe_overlay::overlay::run(game_pid),
        Commands::Trigger { action } => match poe_overlay::running::send(action) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("poe-overlay: {err}");
                ExitCode::FAILURE
            }
        },
        Commands::Setup { what: Setup::Hotkeys { remove } } => poe_overlay::hotkeys::run_setup(remove),
        Commands::Credits => {
            print!("{}", poe_overlay::poe1::route::CREDITS);
            ExitCode::SUCCESS
        }
    }
}
