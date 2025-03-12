use crate::library::snapshot::{restore, snapshot};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Snapshot {
        #[arg(long, short, default_value_t = false)]
        compress: bool,

        source_path: PathBuf,
        snapshot_path: PathBuf,
    },
    Restore {
        snapshot_path: PathBuf,
        restore_path: PathBuf,
    },
}

pub fn run() -> anyhow::Result<()> {
    let args = Cli::parse();

    match args.command {
        Commands::Snapshot {
            compress,
            source_path,
            snapshot_path,
        } => snapshot(&source_path, &snapshot_path, compress)?,
        Commands::Restore {
            snapshot_path,
            restore_path,
        } => restore(&snapshot_path, &restore_path)?,
    }

    Ok(())
}
