use clap::{Parser, Subcommand};

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
        compression: bool,

        /// Name of the volume to snapshot.
        volume_name: String,

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
