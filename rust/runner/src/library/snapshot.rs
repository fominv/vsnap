use std::path::{Path, PathBuf};

use anyhow::Result;
use async_compression::{
    Level,
    tokio::{bufread::ZstdDecoder, write::ZstdEncoder},
};
use futures_util::TryStreamExt;
use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufReader, BufStream, BufWriter},
    sync::mpsc::{self, Sender},
};
use tokio_tar::Builder;

use crate::library::{
    constant::{SNAPSHOT_METADATA, SNAPSHOT_TAR, SNAPSHOT_TAR_ZST},
    metadata::SnapshotMetadata,
    progress::{AsyncProgressReaderWriter, ProgressReporter},
};

pub async fn snapshot(source_path: &Path, snapshot_path: &Path, compress: bool) -> Result<()> {
    let (sender, receiver) = mpsc::channel::<u64>(100);
    let total_size = calculate_total_size(source_path.to_path_buf()).await?;

    ProgressReporter::new(receiver, total_size).listen();

    SnapshotMetadata::new(total_size)
        .write(&snapshot_path.join(SNAPSHOT_METADATA))
        .await?;

    match compress {
        true => {
            let tar_file = File::create(&snapshot_path.join(SNAPSHOT_TAR_ZST)).await?;
            compress_dir(source_path, tar_file, sender).await?;
        }
        false => {
            let tar_file = File::create(&snapshot_path.join(SNAPSHOT_TAR)).await?;
            tar_dir(source_path, tar_file, sender).await?;
        }
    }

    Ok(())
}

pub async fn restore(snapshot_path: &Path, restore_path: &Path) -> Result<()> {
    let (sender, receiver) = mpsc::channel::<u64>(100);

    let total_size = SnapshotMetadata::read(&snapshot_path.join(SNAPSHOT_METADATA))
        .await?
        .total_size;

    ProgressReporter::new(receiver, total_size).listen();

    let is_compressed = snapshot_path.join(SNAPSHOT_TAR_ZST).exists();

    match is_compressed {
        true => {
            let tar_file = File::open(&snapshot_path.join(SNAPSHOT_TAR_ZST)).await?;
            decompress_dir(tar_file, restore_path, sender).await?;
        }
        false => {
            let tar_file = File::open(&snapshot_path.join(SNAPSHOT_TAR)).await?;
            untar_dir(tar_file, restore_path, sender).await?;
        }
    }

    Ok(())
}

async fn calculate_total_size(path: PathBuf) -> Result<u64> {
    tokio::task::spawn_blocking(|| {
        let total_size = walkdir::WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.metadata().ok())
            .filter_map(|m| m.is_file().then(|| m.len()))
            .sum();

        Ok(total_size)
    })
    .await?
}

async fn compress_dir(
    dir_to_compress: &Path,
    tar_file: tokio::fs::File,
    sender: Sender<u64>,
) -> anyhow::Result<()> {
    let buffered_writer = BufWriter::new(tar_file);
    let encoder = ZstdEncoder::with_quality(buffered_writer, Level::Default);
    let progress_reporter = AsyncProgressReaderWriter::new(encoder, sender);

    let mut archive = Builder::new(progress_reporter);
    archive.append_dir_all("./", dir_to_compress).await?;

    let mut encoder = archive.into_inner().await?;
    encoder.shutdown().await?;

    Ok(())
}

async fn decompress_dir(
    tar_file: tokio::fs::File,
    restore_path: &Path,
    sender: Sender<u64>,
) -> anyhow::Result<()> {
    let buffered_reader = BufReader::new(tar_file);
    let decoder = ZstdDecoder::new(buffered_reader);
    let progress_reporter = AsyncProgressReaderWriter::new(decoder, sender);

    let mut archive = tokio_tar::Archive::new(progress_reporter);
    archive
        .entries()?
        .try_for_each(async |mut entry| {
            let path = restore_path.join(entry.path()?);
            entry.unpack(path).await?;

            Ok(())
        })
        .await?;

    Ok(())
}

async fn tar_dir(dir: &Path, tar_file: tokio::fs::File, sender: Sender<u64>) -> anyhow::Result<()> {
    let buffered_writer = BufWriter::new(tar_file);
    let progress_reporter = AsyncProgressReaderWriter::new(buffered_writer, sender);

    let mut archive = Builder::new(progress_reporter);

    archive.append_dir_all("./", dir).await?;

    let mut encoder = archive.into_inner().await?;
    encoder.shutdown().await?;

    Ok(())
}

async fn untar_dir(
    tar_file: tokio::fs::File,
    restore_path: &Path,
    sender: Sender<u64>,
) -> anyhow::Result<()> {
    let buffered_reader = BufStream::new(tar_file);
    let progress_reporter = AsyncProgressReaderWriter::new(buffered_reader, sender);

    let mut archive = tokio_tar::Archive::new(progress_reporter);

    archive
        .entries()?
        .try_for_each(async |mut entry| {
            let path = restore_path.join(entry.path()?);
            entry.unpack(path).await?;

            Ok(())
        })
        .await?;

    Ok(())
}
