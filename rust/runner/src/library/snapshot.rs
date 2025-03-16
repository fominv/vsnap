use std::path::{Path, PathBuf};

use anyhow::Result;
use async_compression::{
    Level,
    tokio::{bufread::ZstdDecoder, write::ZstdEncoder},
};
use futures_util::{StreamExt, TryStreamExt};
use tokio::{
    fs::File,
    io::BufStream,
    sync::mpsc::{self, Sender},
};
use tokio_tar::Builder;
use walkdir::WalkDir;

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
    let buffered_stream = BufStream::new(tar_file);
    let encoder = ZstdEncoder::with_quality(buffered_stream, Level::Default);
    // let progress_writer = AsyncProgressReaderWriter::new(encoder, sender);

    let mut archive = Builder::new(encoder);
    // archive.append_dir_all(".", dir_to_compress).await?;
    // archive.finish().await?;

    for entry in WalkDir::new(&dir_to_compress)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let relative_path = entry.path().strip_prefix(&dir_to_compress)?;

        if relative_path == Path::new("") {
            continue;
        }

        let file_type = entry.file_type();

        if file_type.is_symlink() {
            // let target = std::fs::read_link(entry.path())?;

            // let mut header = Header::new_gnu();
            // header.set_entry_type(EntryType::Symlink);
            // header.set_size(0);

            // tar_builder.append_link(&mut header, relative_path, &target)?;

            continue;
        } else if file_type.is_dir() {
            archive.append_dir(relative_path, entry.path()).await?;
        } else if file_type.is_file() {
            let mut file = File::open(entry.path()).await?;
            archive.append_file(relative_path, &mut file).await?;
        }
    }

    archive.finish().await?;

    Ok(())
}

async fn decompress_dir(
    tar_file: tokio::fs::File,
    restore_path: &Path,
    sender: Sender<u64>,
) -> anyhow::Result<()> {
    let buffered_stream = BufStream::new(tar_file);
    let decoder = ZstdDecoder::new(buffered_stream);

    // let progress_writer = AsyncProgressReaderWriter::new(decoder, sender);
    let mut archive = tokio_tar::Archive::new(decoder);

    println!("Restoring to {:?}", restore_path);

    archive
        .entries()?
        .try_for_each_concurrent(10, |entry| async {
            let mut entry = entry;
            let path = restore_path.join(entry.path()?);
            entry.unpack(&path).await?;

            Ok(())
        })
        .await?;

    println!("Restored to {:?}", restore_path);

    Ok(())
}

async fn tar_dir(dir: &Path, tar_file: tokio::fs::File, sender: Sender<u64>) -> anyhow::Result<()> {
    let buffered_stream = BufStream::new(tar_file);
    // let progress_writer = AsyncProgressReaderWriter::new(buffered_stream, sender);

    let mut archive = Builder::new(buffered_stream);

    archive.append_dir_all(".", dir).await?;
    archive.finish().await?;

    Ok(())
}

async fn untar_dir(
    tar_file: tokio::fs::File,
    restore_path: &Path,
    sender: Sender<u64>,
) -> anyhow::Result<()> {
    let buffered_stream = BufStream::new(tar_file);
    let decoder = ZstdDecoder::new(buffered_stream);

    // let progress_writer = AsyncProgressReaderWriter::new(decoder, sender);

    let mut archive = tokio_tar::Archive::new(decoder);

    println!("Restoring to {:?}", restore_path);

    archive
        .entries()?
        .try_for_each_concurrent(10, |entry| async {
            let mut entry = entry;
            let path = restore_path.join(entry.path()?);
            entry.unpack(&path).await?;

            Ok(())
        })
        .await?;

    println!("Restored to {:?}", restore_path);

    Ok(())
}
