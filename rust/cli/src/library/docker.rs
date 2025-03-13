use anyhow::anyhow;
use bollard::{
    Docker,
    container::{Config, CreateContainerOptions, StartContainerOptions, WaitContainerOptions},
    secret::{HostConfig, Mount},
    volume::CreateVolumeOptions,
};
use futures::StreamExt;
use vsnap_library::VERSION;

pub async fn verify_source_volume(docker: &Docker, source_volume_name: &str) -> anyhow::Result<()> {
    // Check if the volume exists.
    let volume = match docker.inspect_volume(&source_volume_name).await.ok() {
        Some(volume) => volume,
        None => {
            return Err(anyhow!("Volume {} does not exist.", source_volume_name));
        }
    };

    // Check if the volume is in use.
    if let Some(usage) = volume.usage_data {
        if usage.ref_count > 0 {
            return Err(anyhow!("Volume {} is in use.", source_volume_name));
        }
    }

    Ok(())
}

pub async fn setup_snapshot_volume(
    docker: &Docker,
    snapshot_volume_name: &str,
) -> anyhow::Result<()> {
    // Create a new volume for the snapshot.
    let config = CreateVolumeOptions {
        name: snapshot_volume_name,
        ..Default::default()
    };

    docker.create_volume(config).await?;

    Ok(())
}

pub async fn snapshot(
    docker: &Docker,
    source_volume_name: &str,
    snapshot_volume_name: &str,
    compress: bool,
) -> anyhow::Result<()> {
    const SOURCE_DIR: &str = "/mnt/source";
    const SNAPSHOT_DIR: &str = "/mnt/snapshot";

    let container_name = format!("vsnap-{}", chrono::Utc::now().timestamp());

    let options = Some(CreateContainerOptions {
        name: container_name.clone(),
        platform: None,
    });

    let mut cmd = vec!["snapshot"];

    if compress {
        cmd.push("--compress");
    }

    cmd.extend(vec![SOURCE_DIR, SNAPSHOT_DIR]);

    let host_config = HostConfig {
        mounts: Some(vec![
            Mount {
                source: Some(source_volume_name.to_string()),
                target: Some(SOURCE_DIR.to_string()),
                typ: Some(bollard::secret::MountTypeEnum::VOLUME),
                read_only: Some(true),
                ..Default::default()
            },
            Mount {
                source: Some(snapshot_volume_name.to_string()),
                target: Some(SNAPSHOT_DIR.to_string()),
                typ: Some(bollard::secret::MountTypeEnum::VOLUME),
                ..Default::default()
            },
        ]),
        ..Default::default()
    };

    let image = format!("vsnap:{}", VERSION);
    let config = Config {
        image: Some(image.as_str()),
        cmd: Some(cmd),
        host_config: Some(host_config),
        // TODO: Add tty support
        // tty: Some(true),
        // open_stdin: Some(true),
        // stdin_once: Some(true),
        ..Default::default()
    };

    match (async || {
        docker.create_container(options, config).await?;

        docker
            .start_container(&container_name, None::<StartContainerOptions<String>>)
            .await?;

        docker
            .wait_container(&container_name, None::<WaitContainerOptions<String>>)
            .for_each(async |_| {})
            .await;

        Ok::<(), anyhow::Error>(())
    })()
    .await
    {
        result => {
            docker.remove_container(&container_name, None).await?;

            result
        }
    }
}
