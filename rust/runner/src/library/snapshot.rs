use std::{fs::File, path::Path, sync};

use anyhow::Result;
use tar::{Archive, Builder};
use zstd::Encoder;

use crate::library::{
    constant::{SNAPSHOT_METADATA, SNAPSHOT_TAR, SNAPSHOT_TAR_ZST},
    metadata::SnapshotMetadata,
    progress::{ProgressListener, ProgressReporterReader, ProgressReporterWriter},
};

pub fn snapshot(source_path: &Path, snapshot_path: &Path, compress: bool) -> Result<()> {
    let (sender, receiver) = sync::mpsc::channel::<u64>();
    let total_size = calculate_total_size(source_path)?;

    SnapshotMetadata::new(total_size).write(&snapshot_path.join(SNAPSHOT_METADATA))?;
    ProgressListener::new(total_size, receiver).listen();

    match compress {
        true => {
            let tar_file = File::create(&snapshot_path.join(SNAPSHOT_TAR_ZST))?;
            compress_dir(source_path, tar_file, sender)?;
        }
        false => {
            let tar_file = File::create(snapshot_path.join(SNAPSHOT_TAR))?;
            tar_dir(source_path, tar_file, sender)?;
        }
    }

    Ok(())
}

pub fn restore(snapshot_path: &Path, restore_path: &Path) -> Result<()> {
    let (sender, receiver) = sync::mpsc::channel::<u64>();
    let total_size = SnapshotMetadata::read(&snapshot_path.join(SNAPSHOT_METADATA))?.total_size;

    ProgressListener::new(total_size, receiver).listen();

    let is_compressed = snapshot_path.join(SNAPSHOT_TAR_ZST).exists();

    match is_compressed {
        true => {
            let tar_file = File::open(snapshot_path.join(SNAPSHOT_TAR_ZST))?;
            decompress_tar(tar_file, restore_path, sender)?;
        }
        false => {
            let tar_file = File::open(snapshot_path.join(SNAPSHOT_TAR))?;
            untar_dir(tar_file, restore_path, sender)?;
        }
    }

    Ok(())
}

fn calculate_total_size(path: &Path) -> Result<u64> {
    let total_size = walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter_map(|m| m.is_file().then(|| m.len()))
        .sum();

    Ok(total_size)
}

fn compress_dir(
    dir_to_compress: &Path,
    tar_file: File,
    sender: sync::mpsc::Sender<u64>,
) -> anyhow::Result<()> {
    let buffered_writer = std::io::BufWriter::new(tar_file);
    let mut encoder = Encoder::new(buffered_writer, 0)?.auto_finish();
    let mut archive = Builder::new(ProgressReporterWriter::new(&mut encoder, sender));

    archive.append_dir_all("./", dir_to_compress)?;
    archive.finish()?;

    Ok(())
}

fn decompress_tar(
    tar_file: File,
    destination_dir: &Path,
    sender: sync::mpsc::Sender<u64>,
) -> anyhow::Result<()> {
    let buffered_reader = std::io::BufReader::new(tar_file);
    let mut decoder = zstd::Decoder::new(buffered_reader)?;
    let mut archive = Archive::new(ProgressReporterReader::new(&mut decoder, sender));

    archive.unpack(destination_dir)?;

    Ok(())
}

fn tar_dir(
    dir_to_tar: &Path,
    tar_file: File,
    sender: sync::mpsc::Sender<u64>,
) -> anyhow::Result<()> {
    let buffered_writer = std::io::BufWriter::new(tar_file);
    let mut archive = Builder::new(ProgressReporterWriter::new(buffered_writer, sender));

    archive.append_dir_all("./", dir_to_tar)?;
    archive.finish()?;

    Ok(())
}

fn untar_dir(
    tar_file: File,
    destination_dir: &Path,
    sender: sync::mpsc::Sender<u64>,
) -> anyhow::Result<()> {
    let buffered_reader = std::io::BufReader::new(tar_file);
    let mut archive = Archive::new(ProgressReporterReader::new(buffered_reader, sender));

    archive.unpack(destination_dir)?;

    Ok(())
}
