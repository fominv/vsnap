use std::{
    fs::{self, File},
    io::{self, BufRead, Read, Write},
    path::Path,
};

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use tar::{Archive, Builder};
use zstd::{Encoder, stream::read::Decoder};

use crate::library::constant::{SNAPSHOT_SUB_DIR, SNAPSHOT_TAR_ZST};

pub fn snapshot(source_path: &Path, snapshot_path: &Path, compress: bool) -> Result<()> {
    let total_size = calculate_total_size(source_path)?;
    let pb = create_progress_bar(total_size)?;

    match compress {
        true => {
            let tar_file = File::create(&snapshot_path.join(SNAPSHOT_TAR_ZST))?;

            let buffered_writer = std::io::BufWriter::new(tar_file);
            let mut encoder = Encoder::new(buffered_writer, 0)?.auto_finish();
            let mut tar_builder = Builder::new(ProgressWriter::new(&mut encoder, &pb));

            for entry in walkdir::WalkDir::new(source_path).follow_links(false) {
                let entry = entry?;
                let relative_path = entry.path().strip_prefix(source_path)?;

                if relative_path == Path::new("") {
                    continue;
                }

                let file_type = entry.file_type();

                if file_type.is_symlink() {
                    // TODO: Do not follow symlinks
                } else if file_type.is_dir() {
                    tar_builder.append_dir(relative_path, entry.path())?;
                } else if file_type.is_file() {
                    let mut file = File::open(entry.path())?;
                    tar_builder.append_file(relative_path, &mut file)?;
                }
            }

            tar_builder.finish()?;
        }
        false => {
            let files_path = snapshot_path.join(SNAPSHOT_SUB_DIR);
            fs::create_dir_all(&files_path)?;

            for entry in walkdir::WalkDir::new(source_path).follow_links(false) {
                let entry = entry?;
                let relative_path = entry.path().strip_prefix(source_path)?;
                let destination_path = files_path.join(relative_path);

                let file_type = entry.file_type();

                if file_type.is_symlink() {
                    // TODO: Do not follow symlinks
                } else if file_type.is_dir() {
                    fs::create_dir_all(&destination_path)?;
                } else if file_type.is_file() {
                    let mut source_file = File::open(entry.path())?;
                    let mut dest_file = File::create(&destination_path)?;
                    copy_with_progress(&mut source_file, &mut dest_file, &pb)?;
                }
            }
        }
    }

    Ok(())
}

pub fn restore(snapshot_path: &Path, restore_path: &Path) -> Result<()> {
    let is_compressed = snapshot_path.join(SNAPSHOT_TAR_ZST).exists();

    if is_compressed {
        let snapshot_file_path = snapshot_path.join(SNAPSHOT_TAR_ZST);
        let snapshot_file = File::open(&snapshot_file_path).with_context(|| {
            format!(
                "Failed to open snapshot file: {}",
                snapshot_file_path.display()
            )
        })?;

        let compressed_size = snapshot_file.metadata()?.len();
        let pb = create_progress_bar(compressed_size)?;

        let buffered_reader = std::io::BufReader::new(ProgressReader::new(snapshot_file, &pb));
        let mut decoder =
            Decoder::new(buffered_reader).with_context(|| "Failed to create zstd decoder")?;

        let total_size = calculate_uncompressed_size(&mut decoder)?;
        let pb_unpack = create_progress_bar(total_size)?;

        let snapshot_file = File::open(&snapshot_file_path)?;
        let buffered_reader = std::io::BufReader::new(ProgressReader::new(snapshot_file, &pb));
        decoder = Decoder::new(buffered_reader)?;

        let mut archive = Archive::new(ProgressReader::new(&mut decoder, &pb_unpack));

        for entry in archive.entries()? {
            let mut entry = entry.context("Error reading tar entry")?;
            let path = restore_path.join(entry.path()?);
            entry.unpack(&path).context("Error unpacking tar entry")?;
        }

        pb_unpack.finish_with_message("Unpacking complete!");
        pb.finish_with_message("Decompression complete!");
    } else {
        let files_path = snapshot_path.join(SNAPSHOT_SUB_DIR);
        let pb = create_progress_bar(calculate_total_size(&files_path)?)?;

        for entry in walkdir::WalkDir::new(&files_path).follow_links(false) {
            let entry = entry?;
            let relative_path = entry.path().strip_prefix(&files_path)?;
            let destination_path = restore_path.join(relative_path);

            let file_type = entry.file_type();

            if file_type.is_symlink() {
                // TODO: Do not follow symlinks
            } else if file_type.is_dir() {
                fs::create_dir_all(&destination_path)?;
            } else if file_type.is_file() {
                let mut src_file = File::open(entry.path())?;
                let mut dest_file = File::create(&destination_path)?;
                copy_with_progress(&mut src_file, &mut dest_file, &pb)?;
            }
        }

        pb.finish_with_message("Restore complete!");
    }

    Ok(())
}

fn create_progress_bar(total_size: u64) -> anyhow::Result<ProgressBar> {
    let pb = ProgressBar::new(total_size);

    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")? 
            .progress_chars("#>-"),
    );

    Ok(pb)
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

fn calculate_uncompressed_size<R: Read>(decoder: &mut Decoder<'_, R>) -> Result<u64>
where
    R: BufRead,
{
    let mut buffer = [0; 8192];
    let mut total_size = 0;

    loop {
        let bytes_read = decoder.read(&mut buffer)?;
        total_size += bytes_read as u64;
        if bytes_read == 0 {
            break;
        }
    }

    Ok(total_size)
}

fn copy_with_progress<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    pb: &ProgressBar,
) -> Result<()> {
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;

        if bytes_read == 0 {
            break;
        }

        writer.write_all(&buffer[..bytes_read])?;
        pb.inc(bytes_read as u64);
    }

    Ok(())
}

struct ProgressWriter<'a, W: Write> {
    inner: W,
    progress_bar: &'a ProgressBar,
}

impl<'a, W: Write> ProgressWriter<'a, W> {
    fn new(inner: W, progress_bar: &'a ProgressBar) -> Self {
        ProgressWriter {
            inner,
            progress_bar,
        }
    }
}

impl<'a, W: Write> Write for ProgressWriter<'a, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = self.inner.write(buf)?;
        self.progress_bar.inc(bytes_written as u64);
        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct ProgressReader<'a, R: Read> {
    inner: R,
    progress_bar: &'a ProgressBar,
}

impl<'a, R: Read> ProgressReader<'a, R> {
    fn new(inner: R, progress_bar: &'a ProgressBar) -> Self {
        ProgressReader {
            inner,
            progress_bar,
        }
    }
}

impl<'a, R: Read> Read for ProgressReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.inner.read(buf)?;
        self.progress_bar.inc(bytes_read as u64);
        Ok(bytes_read)
    }
}
