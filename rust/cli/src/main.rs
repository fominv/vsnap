mod library;

use clap::Parser;
use library::cli::{Cli, Commands};

fn main() {
    let args = Cli::parse();

    match args.command {
        Commands::Create { volume_name } => {
            println!("Creating snapshot for volume: {}", volume_name);
            // TODO: Implement snapshot creation logic.
        }
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
}
