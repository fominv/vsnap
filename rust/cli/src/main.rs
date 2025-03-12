mod library;

use anyhow::Result;
use bollard::{
    Docker,
    container::{Config, CreateContainerOptions, StartContainerOptions},
    models::{HostConfig, Mount},
    volume::CreateVolumeOptions,
};
use clap::Parser;
use library::{
    cli::{Cli, Commands},
    config::Snapshot,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();

    match args.command {
        Commands::Create {
            volume_name,
            compression,
            snapshot_name,
        } => {
            let docker = Docker::connect_with_local_defaults()?;

            let snapshot = Snapshot {
                timestamp: chrono::Utc::now().timestamp(),
                name: snapshot_name.clone(),
            };

            // Check if the volume exists.
            let volume = match docker.inspect_volume(&volume_name).await.ok() {
                Some(volume) => volume,
                None => {
                    eprintln!("Volume {} does not exist.", volume_name);

                    // TODO: Return non zero exit code.
                    return Ok(());
                }
            };

            // Check if the volume is in use.
            if let Some(usage) = volume.usage_data {
                if usage.ref_count > 0 {
                    eprintln!("Volume {} is in use.", volume_name);
                    return Ok(());
                }
            }

            // Create a new volume for the snapshot.
            let config = CreateVolumeOptions {
                name: snapshot.to_volume_name(),
                ..Default::default()
            };

            docker.create_volume(config).await?;

            let container_name = format!("vsnap-{}", chrono::Utc::now().timestamp());

            // TODO: Make sure the container is removed at the end.
            let options = Some(CreateContainerOptions {
                name: container_name.clone(),
                platform: None,
            });

            let mut cmd = vec![
                "--source",
                "/mnt/source",
                "--target",
                "/mnt/target",
                "snapshot",
            ];

            if compression {
                cmd.push("--compression");
            }

            // let cmd = vec!["/mnt/source"];

            let host_config = HostConfig {
                mounts: Some(vec![
                    Mount {
                        source: Some(volume_name),
                        target: Some("/mnt/source".to_string()),
                        typ: Some(bollard::secret::MountTypeEnum::VOLUME),
                        read_only: Some(true),
                        ..Default::default()
                    },
                    Mount {
                        source: Some(snapshot.to_volume_name()),
                        target: Some("/mnt/target".to_string()),
                        typ: Some(bollard::secret::MountTypeEnum::VOLUME),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            };

            // Fix the version in the binary
            let config = Config {
                image: Some("vsnap:latest"),
                cmd: Some(cmd),
                // entrypoint: Some(vec!["ls"]),
                host_config: Some(host_config),
                tty: Some(true),
                open_stdin: Some(true),
                stdin_once: Some(true),
                ..Default::default()
            };

            // Run a command in the container to create a snapshot.
            docker.create_container(options, config).await?;
            docker
                .start_container(&container_name, None::<StartContainerOptions<String>>)
                .await?;
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

    Ok(())
}
