//! `ptrain`: run a trainer in a game's Proton prefix with one command.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use pt_core::AppId;
use pt_execution::{launch, LaunchOptions, Trainer};

#[derive(Parser)]
#[command(
    name = "ptrain",
    version,
    about = "Run game trainers through Proton, without the terminal gymnastics"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a trainer inside the prefix of a Steam game.
    Run {
        /// Steam AppID of the game (e.g. 2322010)
        #[arg(value_parser = clap::value_parser!(AppId))]
        app_id: AppId,
        /// Path to the trainer .exe
        trainer: PathBuf,
        /// Seconds to wait for the game to start (0 = game must already be running)
        #[arg(short, long, default_value_t = 120)]
        wait: u64,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Cmd::Run {
            app_id,
            trainer,
            wait,
        } => match run(app_id, trainer, wait) {
            Ok(code) => code,
            Err(msg) => {
                eprintln!("ptrain: {msg}");
                ExitCode::FAILURE
            }
        },
    }
}

fn run(app_id: AppId, trainer: PathBuf, wait: u64) -> Result<ExitCode, String> {
    let trainer = Trainer::from_path(trainer).map_err(|e| e.safe_message())?;
    let options = LaunchOptions {
        wait_for_game: (wait > 0).then(|| Duration::from_secs(wait)),
        ..LaunchOptions::default()
    };

    let status = launch(app_id, &trainer, options, || {
        eprintln!("ptrain: waiting for game {app_id}... open it in Steam (timeout {wait}s)");
    })
    .map_err(|e| e.safe_message())?;

    Ok(if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(status.code().unwrap_or(1) as u8)
    })
}
