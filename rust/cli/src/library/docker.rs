use std::{collections::HashMap, str};

use anyhow::anyhow;
use bollard::{
    Docker,
    container::{Config, CreateContainerOptions, StartContainerOptions, WaitContainerOptions},
    secret::{HostConfig, Mount},
    volume::{CreateVolumeOptions, ListVolumesOptions},
};
use futures::StreamExt;
use itertools::Itertools;
use regex::Regex;
use vsnap_library::VERSION;

static SNAPSHOT_PREFIX_REGEX: &str = r"^vsnap-\d{10,}-";

pub fn get_snapshot_volume_name(timestamp: i64, name: &str) -> String {
    format!("vsnap-{}-{}", timestamp, name)
}

pub async fn verify_snapshot(docker: &Docker, snapshot_name: &str) -> anyhow::Result<()> {
    if find_snapshot_volume_name_by_snapshot_name(&docker, snapshot_name)
        .await?
        .is_some()
    {
        return Err(anyhow!("Snapshot already exists: {}", snapshot_name));
    }

    Ok(())
}

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

pub async fn create_volume(docker: &Docker, volume_name: &str) -> anyhow::Result<()> {
    docker
        .create_volume(CreateVolumeOptions {
            name: volume_name,
            ..Default::default()
        })
        .await?;

    Ok(())
}

pub async fn drop_volume(docker: &Docker, volume_name: &str) -> anyhow::Result<()> {
    if docker.inspect_volume(&volume_name).await.ok().is_none() {
        return Ok(());
    }

    docker.remove_volume(volume_name, None).await?;

    Ok(())
}

pub async fn find_snapshot_volume_names(docker: &Docker) -> anyhow::Result<Vec<String>> {
    let volumes = docker
        .list_volumes(Some(ListVolumesOptions {
            filters: HashMap::<&str, Vec<&str>>::from([("name", vec!["vsnap-"])]),
        }))
        .await?;

    let re = Regex::new(SNAPSHOT_PREFIX_REGEX)?;

    Ok(volumes
        .volumes
        .map(|volumes| {
            volumes
                .into_iter()
                .filter(|volume| re.is_match(&volume.name))
                .map(|v| v.name)
                .collect::<Vec<String>>()
        })
        .unwrap_or(vec![]))
}

pub fn strip_snapshot_prefix(volume_name: &str) -> anyhow::Result<String> {
    let re = Regex::new(SNAPSHOT_PREFIX_REGEX)?;
    Ok(re.replace(volume_name, "").to_string())
}

pub async fn find_snapshot_volume_name_by_snapshot_name(
    docker: &Docker,
    snapshot_name: &str,
) -> anyhow::Result<Option<String>> {
    let volume_names = find_snapshot_volume_names(docker).await?;

    let volume_names = volume_names.into_iter().filter(|volume_name| {
        strip_snapshot_prefix(&volume_name)
            .map(|x| x == snapshot_name)
            .unwrap_or(false)
    });

    volume_names.at_most_one().map_err(|_| {
        anyhow!(
            "More than one snapshot with the same name: {}",
            snapshot_name
        )
    })
}

async fn run_command(
    docker: &Docker,
    container_name: &str,
    cmd: Vec<&str>,
    host_config: HostConfig,
) -> anyhow::Result<()> {
    let image = format!("vsnap:{}", VERSION);

    let options = Some(CreateContainerOptions {
        name: container_name.to_string(),
        platform: None,
    });

    let config = Config {
        image: Some(image.as_str()),
        cmd: Some(cmd),
        host_config: Some(host_config),
        // TODO: Add tty support
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

pub async fn snapshot(
    docker: &Docker,
    source_volume_name: &str,
    snapshot_volume_name: &str,
    compress: bool,
) -> anyhow::Result<()> {
    const SOURCE_DIR: &str = "/mnt/source";
    const SNAPSHOT_DIR: &str = "/mnt/snapshot";

    let container_name = format!("vsnap-{}", chrono::Utc::now().timestamp());

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

    run_command(docker, &container_name, cmd, host_config).await
}

pub async fn restore_snapshot(
    docker: &Docker,
    snapshot_volume_name: &str,
    restore_volume_name: &str,
) -> anyhow::Result<()> {
    const SNAPSHOT_DIR: &str = "/mnt/snapshot";
    const RESTORE_DIR: &str = "/mnt/restore";

    let container_name = format!("vsnap-{}", chrono::Utc::now().timestamp());

    let mut cmd = vec!["restore"];

    cmd.extend(vec![SNAPSHOT_DIR, RESTORE_DIR]);

    let host_config = HostConfig {
        mounts: Some(vec![
            Mount {
                source: Some(snapshot_volume_name.to_string()),
                target: Some(SNAPSHOT_DIR.to_string()),
                typ: Some(bollard::secret::MountTypeEnum::VOLUME),
                read_only: Some(true),
                ..Default::default()
            },
            Mount {
                source: Some(restore_volume_name.to_string()),
                target: Some(RESTORE_DIR.to_string()),
                typ: Some(bollard::secret::MountTypeEnum::VOLUME),
                ..Default::default()
            },
        ]),
        ..Default::default()
    };

    run_command(docker, &container_name, cmd, host_config).await?;

    Ok(())
}
