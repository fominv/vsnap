use std::{collections::HashMap, str, sync::LazyLock, time::Duration};

use anyhow::anyhow;
use bollard::{
    Docker,
    container::{
        Config, CreateContainerOptions, ListContainersOptions, StartContainerOptions,
        WaitContainerOptions,
    },
    image::CreateImageOptions,
    secret::{HostConfig, Mount},
    volume::{CreateVolumeOptions, ListVolumesOptions},
};
use chrono::Local;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use regex::Regex;
use vsnap_library::VERSION;

pub static SNAPSHOT_PREFIX_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^vsnap-(\d{10,})-").expect("Failed to compile snapshot prefix regex")
});

pub fn get_snapshot_volume_name(timestamp: i64, name: &str) -> String {
    format!("vsnap-{}-{}", timestamp, name)
}

pub async fn verify_volume_not_in_use(docker: &Docker, volume_name: &str) -> anyhow::Result<()> {
    let containers = docker
        .list_containers(Some(ListContainersOptions::<&str> {
            all: true,
            filters: HashMap::<&str, Vec<&str>>::from([("volume", vec![volume_name])]),
            ..Default::default()
        }))
        .await?;

    let container_names = containers
        .iter()
        .filter_map(|container| {
            container
                .names
                .iter()
                .flatten()
                .next()
                .map(|name| name.strip_prefix("/"))
                .flatten()
                .map(|name| name.to_string())
        })
        .collect::<Vec<String>>();

    if container_names.len() > 0 {
        return Err(anyhow!(
            "Volume is in use by {}",
            container_names.join(", ")
        ));
    }

    Ok(())
}

pub async fn verify_snapshot_does_not_exist(
    docker: &Docker,
    snapshot_name: &str,
) -> anyhow::Result<()> {
    if find_snapshot_volume_name_by_snapshot_name(&docker, snapshot_name)
        .await?
        .is_some()
    {
        return Err(anyhow!("Snapshot already exists: {}", snapshot_name));
    }

    Ok(())
}

pub async fn volume_exists(docker: &Docker, volume_name: &str) -> bool {
    docker.inspect_volume(&volume_name).await.ok().is_some()
}

pub async fn verify_volume_exists(docker: &Docker, volume_name: &str) -> anyhow::Result<()> {
    if !volume_exists(docker, volume_name).await {
        return Err(anyhow!("Volume does not exist: {}", volume_name));
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
    verify_volume_not_in_use(docker, volume_name).await?;
    docker.remove_volume(volume_name, None).await?;

    Ok(())
}

pub async fn find_snapshot_volume_names(docker: &Docker) -> anyhow::Result<Vec<String>> {
    let volumes = docker
        .list_volumes(Some(ListVolumesOptions {
            filters: HashMap::<&str, Vec<&str>>::from([("name", vec!["vsnap-"])]),
        }))
        .await?;

    Ok(volumes
        .volumes
        .map(|volumes| {
            volumes
                .into_iter()
                .filter(|volume| SNAPSHOT_PREFIX_REGEX.is_match(&volume.name))
                .map(|v| v.name)
                .collect::<Vec<String>>()
        })
        .unwrap_or(vec![]))
}

pub enum VolumeSize {
    Bytes(i64),
    Unavailable,
}

pub async fn get_volume_sizes_for_volume_names(
    docker: &Docker,
    volume_names: &Vec<String>,
) -> anyhow::Result<HashMap<String, VolumeSize>> {
    let mut volume_sizes = HashMap::new();

    let df = docker.df().await?;
    for volume in df.volumes.unwrap_or(vec![]) {
        if !volume_names.contains(&volume.name) {
            continue;
        }

        let size = volume
            .usage_data
            .map(|usage_data| VolumeSize::Bytes(usage_data.size))
            .unwrap_or(VolumeSize::Unavailable);

        volume_sizes.insert(volume.name, size);
    }

    Ok(volume_sizes)
}

pub fn extract_snapshot_datetime(volume_name: &str) -> anyhow::Result<chrono::NaiveDateTime> {
    let captures = SNAPSHOT_PREFIX_REGEX.captures(volume_name).ok_or(anyhow!(
        "Failed to extract timestamp from volume name: {}",
        volume_name
    ))?;

    let timestamp = captures.get(1).unwrap().as_str().parse::<i64>()?;

    Ok(chrono::DateTime::from_timestamp(timestamp, 0)
        .ok_or(anyhow!(
            "Failed to parse timestamp from volume name: {}",
            volume_name
        ))?
        .with_timezone(&Local)
        .naive_local())
}

pub fn strip_snapshot_prefix(volume_name: &str) -> String {
    SNAPSHOT_PREFIX_REGEX.replace(volume_name, "").to_string()
}

pub async fn find_snapshot_volume_name_by_snapshot_name(
    docker: &Docker,
    snapshot_name: &str,
) -> anyhow::Result<Option<String>> {
    let volume_names = find_snapshot_volume_names(docker).await?;

    let volume_names = volume_names
        .into_iter()
        .filter(|volume_name| strip_snapshot_prefix(&volume_name) == snapshot_name);

    volume_names.at_most_one().map_err(|_| {
        anyhow!(
            "More than one snapshot with the same name: {}",
            snapshot_name
        )
    })
}

pub async fn image_exists(docker: &Docker, image: &str) -> bool {
    docker.inspect_image(image).await.ok().is_some()
}

pub async fn pull_image(docker: &Docker, image: &str) -> anyhow::Result<()> {
    let pb = ProgressBar::new_spinner();
    pb.set_message("Downloading & Extracting Image...");
    pb.enable_steady_tick(Duration::from_millis(120));
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")?.tick_strings(&[
            "▹▹▹▹▹",
            "▸▹▹▹▹",
            "▹▸▹▹▹",
            "▹▹▸▹▹",
            "▹▹▹▸▹",
            "▹▹▹▹▸",
            "▪▪▪▪▪",
        ]),
    );

    docker
        .create_image(
            Some(CreateImageOptions {
                from_image: image,
                ..Default::default()
            }),
            None,
            None,
        )
        .for_each(async |v| {
            println!("{:?}", v);
        })
        .await;

    pb.finish_with_message("Done");

    Ok(())
}

async fn run_command(
    docker: &Docker,
    cmd: Vec<&str>,
    host_config: HostConfig,
) -> anyhow::Result<()> {
    let container_name = format!("vsnap-{}", chrono::Utc::now().timestamp());
    let image = format!("fominv/vsnap:{}", VERSION.as_str());

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

    if !image_exists(docker, &image).await {
        pull_image(docker, &image).await?;
    }

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

    run_command(docker, cmd, host_config).await
}

pub async fn restore_snapshot(
    docker: &Docker,
    snapshot_volume_name: &str,
    restore_volume_name: &str,
) -> anyhow::Result<()> {
    const SNAPSHOT_DIR: &str = "/mnt/snapshot";
    const RESTORE_DIR: &str = "/mnt/restore";

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

    run_command(docker, cmd, host_config).await?;

    Ok(())
}
