use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::library::snapshot::{restore, snapshot};

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

pub async fn run() -> anyhow::Result<()> {
    if !cfg!(unix) {
        return Err(anyhow::anyhow!(
            "This program is only supported on Unix systems"
        ));
    }

    let args = Cli::parse();

    match args.command {
        Commands::Snapshot {
            compress,
            source_path,
            snapshot_path,
        } => snapshot(&source_path, &snapshot_path, compress).await?,
        Commands::Restore {
            snapshot_path,
            restore_path,
        } => restore(&snapshot_path, &restore_path).await?,
    }

    Ok(())
}
