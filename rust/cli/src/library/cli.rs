use bollard::Docker;
use clap::{Parser, Subcommand};

use crate::library::docker::{setup_snapshot_volume, snapshot, verify_source_volume};

/// A CLI tool for managing Docker volume snapshots.
#[derive(Parser, Debug)]
#[command(name = "vs")]
#[command(about = "Docker Volume Snapshot Tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Subcommands for the vs tool.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create a snapshot of a Docker volume.
    Create {
        /// Whether to compress the snapshot.
        #[arg(long, short, default_value_t = false)]
        compress: bool,

        /// Name of the volume to snapshot.
        source_volume_name: String,

        /// Name of the snapshot.
        snapshot_name: String,
    },
    /// List all snapshots.
    List,
    /// Restore a volume from a snapshot.
    Restore {
        /// Name of the snapshot to restore.
        snapshot_name: String,
    },
    /// Delete a snapshot.
    Delete {
        /// Name of the snapshot to delete.
        snapshot_name: String,
    },
}

pub async fn run() -> anyhow::Result<()> {
    let args = Cli::parse();

    match args.command {
        Commands::Create {
            source_volume_name,
            compress,
            snapshot_name,
        } => create(source_volume_name, snapshot_name, compress).await?,
        Commands::List => {
            println!("Listing all snapshots...");
            // TODO: Implement snapshot listing logic.
        }
        Commands::Restore { snapshot_name } => {
            println!("Restoring volume from snapshot: {}", snapshot_name);
            // TODO: Implement snapshot restoration logic.
        }
        Commands::Delete { snapshot_name } => {
            println!("Deleting snapshot: {}", snapshot_name);
            // TODO: Implement snapshot deletion logic.
        }
    }

    Ok(())
}

async fn create(
    source_volume_name: String,
    snapshot_name: String,
    compress: bool,
) -> anyhow::Result<()> {
    let docker = Docker::connect_with_local_defaults()?;

    let snapshot_volume_name =
        get_snapshot_volume_name(chrono::Utc::now().timestamp(), &snapshot_name);

    verify_source_volume(&docker, &source_volume_name).await?;
    setup_snapshot_volume(&docker, &snapshot_volume_name).await?;
    snapshot(
        &docker,
        &source_volume_name,
        &snapshot_volume_name,
        compress,
    )
    .await?;

    Ok(())
}

fn get_snapshot_volume_name(timestamp: i64, name: &str) -> String {
    format!("vsnap-{}-{}", timestamp, name)
}
