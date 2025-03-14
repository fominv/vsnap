use anyhow::anyhow;
use bollard::Docker;
use clap::{Parser, Subcommand};
use inquire::Confirm;
use vsnap_library::VERSION;

use crate::library::{
    docker::{
        create_volume, drop_volume, find_snapshot_volume_name_by_snapshot_name,
        find_snapshot_volume_names, get_snapshot_volume_name, get_volume_sizes_for_volume_names,
        pull_image, restore_snapshot, snapshot, verify_snapshot_does_not_exist,
        verify_volume_exists, verify_volume_not_in_use, volume_exists,
    },
    table::print_snapshot_table,
};

#[derive(Parser, Debug)]
#[command(
    name = "vsnap",
    bin_name = "vsnap", 
    version = VERSION.as_str(),
    about = indoc::indoc! {
        "vsnap - a docker volume snapshot tool

        If you find this tool useful, feel free to ⭐ the repository on GitHub: https://github.com/fominv/vsnap.git"
    }
)]
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
    List {
        /// Include snapshot sizes.
        /// Might be slow for many / large volumes.
        #[arg(long, short, default_value_t = false)]
        size: bool,
    },
    /// Restore a volume from a snapshot.
    Restore {
        /// Drop after restore.
        #[arg(long, short, default_value_t = false)]
        drop: bool,

        /// Name of the snapshot volume to restore.
        snapshot_name: String,

        /// Name of the volume to restore to.
        restore_volume_name: String,
    },
    /// Drop a snapshot.
    Drop {
        /// Name of the snapshot to delete.
        snapshot_name: String,
    },
}

pub async fn run() -> anyhow::Result<()> {
    let args = Cli::parse();

    match args.command {
        Commands::Create {
            compress,
            source_volume_name,
            snapshot_name,
        } => create(source_volume_name, snapshot_name, compress).await?,
        Commands::List { size } => {
            list(size).await?;
        }
        Commands::Restore {
            drop,
            snapshot_name,
            restore_volume_name,
        } => {
            restore(snapshot_name, restore_volume_name, drop).await?;
        }
        Commands::Drop { snapshot_name } => {
            drop(snapshot_name).await?;
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

    verify_snapshot_does_not_exist(&docker, &snapshot_name).await?;

    let snapshot_volume_name =
        get_snapshot_volume_name(chrono::Utc::now().timestamp(), &snapshot_name);

    verify_volume_not_in_use(&docker, &source_volume_name).await?;
    verify_volume_exists(&docker, &source_volume_name).await?;

    create_volume(&docker, &snapshot_volume_name).await?;

    snapshot(
        &docker,
        &source_volume_name,
        &snapshot_volume_name,
        compress,
    )
    .await?;

    Ok(())
}

async fn list(include_size: bool) -> anyhow::Result<()> {
    let docker = Docker::connect_with_local_defaults()?;

    let mut volume_names = find_snapshot_volume_names(&docker).await?;
    volume_names.sort();

    let volume_sizes = match include_size {
        true => Some(get_volume_sizes_for_volume_names(&docker, &volume_names).await?),
        false => None,
    };

    if volume_names.is_empty() {
        println!("No snapshots found.");
        return Ok(());
    }

    print_snapshot_table(volume_names, volume_sizes)?;

    Ok(())
}

async fn restore(
    snapshot_name: String,
    restore_volume_name: String,
    drop: bool,
) -> anyhow::Result<()> {
    let docker = Docker::connect_with_local_defaults()?;

    let snapshot_volume_name = find_snapshot_volume_name_by_snapshot_name(&docker, &snapshot_name)
        .await?
        .ok_or(anyhow!("Snapshot {} not found", snapshot_name))?;

    if volume_exists(&docker, &restore_volume_name).await {
        let ans = Confirm::new("The volume to restore to already exists, do you wish to drop it?")
            .with_default(false)
            .with_help_message("This will delete the volume and all its data.")
            .prompt();

        if ans? {
            drop_volume(&docker, &restore_volume_name).await?;
        }
    }

    create_volume(&docker, &restore_volume_name).await?;
    restore_snapshot(&docker, &snapshot_volume_name, &restore_volume_name).await?;

    if drop {
        drop_volume(&docker, &snapshot_volume_name).await?;
    }

    Ok(())
}

async fn drop(snapshot_name: String) -> anyhow::Result<()> {
    let docker = Docker::connect_with_local_defaults()?;

    let snapshot_volume_name = find_snapshot_volume_name_by_snapshot_name(&docker, &snapshot_name)
        .await?
        .ok_or(anyhow!("Snapshot {} not found", snapshot_name))?;

    drop_volume(&docker, &snapshot_volume_name).await?;

    Ok(())
}
