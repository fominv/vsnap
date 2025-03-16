use std::{
    cmp::min,
    fs::{self, File},
    io::{self, BufWriter, Read, Seek, Write, stdout},
    path::Path,
};

use anyhow::Result;
use tar::{Archive, Builder, EntryType, Header};
use vsnap::library::Progress;
use zstd::Encoder;

use crate::library::{
    constant::{SNAPSHOT_METADATA, SNAPSHOT_SUB_DIR, SNAPSHOT_TAR_ZST},
    metadata::SnapshotMetadata,
};

fn handle_progress(
    stdout_handle: &mut BufWriter<io::Stdout>,
    total_size: u64,
    progress: &mut u64,
    bytes_written: u64,
) {
    *progress = min(*progress + bytes_written, total_size);

    serde_json::to_string(&Progress {
        progress: *progress,
        total: total_size,
    })
    .ok()
    .map(|x| writeln!(stdout_handle, "{}", x).ok());
}

pub fn snapshot(source_path: &Path, snapshot_path: &Path, compress: bool) -> Result<()> {
    let mut stdout_handle = BufWriter::new(stdout());
    let total_size = calculate_total_size(source_path)?;
    let mut progress = 0;

    SnapshotMetadata::new(total_size).write(&snapshot_path.join(SNAPSHOT_METADATA))?;

    match compress {
        true => {
            let tar_file = File::create(&snapshot_path.join(SNAPSHOT_TAR_ZST))?;
            compress_dir(source_path, tar_file, &mut |bytes_written| {
                handle_progress(&mut stdout_handle, total_size, &mut progress, bytes_written)
            })?;
        }
        false => {
            let files_path = snapshot_path.join(SNAPSHOT_SUB_DIR);
            copy_dir(source_path, &files_path, &mut |bytes_written| {
                handle_progress(&mut stdout_handle, total_size, &mut progress, bytes_written)
            })?;
        }
    }

    Ok(())
}

pub fn restore(snapshot_path: &Path, restore_path: &Path) -> Result<()> {
    let mut stdout_handle = BufWriter::new(stdout());
    let mut progress = 0;

    let metadata = SnapshotMetadata::read(&snapshot_path.join(SNAPSHOT_METADATA))?;

    let is_compressed = snapshot_path.join(SNAPSHOT_TAR_ZST).exists();

    match is_compressed {
        true => {
            let mut tar_file = File::open(snapshot_path.join(SNAPSHOT_TAR_ZST))?;

            tar_file.rewind()?;
            decompress_tar(tar_file, restore_path, &mut |bytes_written| {
                handle_progress(
                    &mut stdout_handle,
                    metadata.total_size,
                    &mut progress,
                    bytes_written,
                )
            })?;
        }
        false => {
            let files_path = snapshot_path.join(SNAPSHOT_SUB_DIR);

            copy_dir(&files_path, restore_path, &mut |bytes_written| {
                handle_progress(
                    &mut stdout_handle,
                    metadata.total_size,
                    &mut progress,
                    bytes_written,
                )
            })?;
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

fn copy_with_progress<R: Read, W: Write, F>(
    reader: &mut R,
    writer: &mut W,
    progress_callback: &mut F,
) -> Result<()>
where
    F: FnMut(u64),
{
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = reader.read(&mut buffer)?;

        if bytes_read == 0 {
            break;
        }

        writer.write_all(&buffer[..bytes_read])?;
        progress_callback(bytes_read as u64);
    }

    Ok(())
}

struct ProgressWriter<W: Write, F>
where
    F: FnMut(u64),
{
    inner: W,
    progress_callback: F,
}

impl<W: Write, F> ProgressWriter<W, F>
where
    F: FnMut(u64),
{
    fn new(inner: W, progress_callback: F) -> Self {
        ProgressWriter {
            inner,
            progress_callback,
        }
    }
}

impl<W: Write, F> Write for ProgressWriter<W, F>
where
    F: FnMut(u64),
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = self.inner.write(buf)?;

        (self.progress_callback)(bytes_written as u64);

        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct ProgressReader<R: Read, F>
where
    F: FnMut(u64),
{
    inner: R,
    progress_callback: F,
}

impl<R: Read, F> ProgressReader<R, F>
where
    F: FnMut(u64),
{
    fn new(inner: R, progress_callback: F) -> Self {
        ProgressReader {
            inner,
            progress_callback,
        }
    }
}

impl<R: Read, F> Read for ProgressReader<R, F>
where
    F: FnMut(u64),
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.inner.read(buf)?;

        (self.progress_callback)(bytes_read as u64);

        Ok(bytes_read)
    }
}

fn compress_dir<F>(
    dir_to_compress: &Path,
    tar_file: File,
    progress_callback: F,
) -> anyhow::Result<()>
where
    F: FnMut(u64),
{
    let buffered_writer = std::io::BufWriter::new(tar_file);
    let mut encoder = Encoder::new(buffered_writer, 0)?.auto_finish();
    let mut tar_builder = Builder::new(ProgressWriter::new(&mut encoder, progress_callback));

    for entry in walkdir::WalkDir::new(dir_to_compress).follow_links(false) {
        let entry = entry?;
        let relative_path = entry.path().strip_prefix(dir_to_compress)?;

        if relative_path == Path::new("") {
            continue;
        }

        let file_type = entry.file_type();

        if file_type.is_symlink() {
            let target = fs::read_link(entry.path())?;

            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Symlink);
            header.set_size(0);

            tar_builder.append_link(&mut header, relative_path, target)?;
        } else if file_type.is_dir() {
            tar_builder.append_dir(relative_path, entry.path())?;
        } else if file_type.is_file() {
            let mut file = File::open(entry.path())?;
            tar_builder.append_file(relative_path, &mut file)?;
        }
    }

    tar_builder.finish()?;

    Ok(())
}

fn decompress_tar<F>(
    tar_file: File,
    destination_dir: &Path,
    progress_callback: F,
) -> anyhow::Result<()>
where
    F: FnMut(u64),
{
    let mut decoder = zstd::Decoder::new(tar_file)?;
    let mut archive = Archive::new(ProgressReader::new(&mut decoder, progress_callback));

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = destination_dir.join(entry.path()?);
        entry.unpack(&path)?;
    }

    Ok(())
}

fn copy_dir<F>(
    dir_to_copy: &Path,
    destination_dir: &Path,
    progress_callback: &mut F,
) -> anyhow::Result<()>
where
    F: FnMut(u64),
{
    fs::create_dir_all(&destination_dir)?;

    for entry in walkdir::WalkDir::new(dir_to_copy).follow_links(false) {
        let entry = entry?;
        let relative_path = entry.path().strip_prefix(dir_to_copy)?;
        let destination_path = destination_dir.join(relative_path);

        let file_type = entry.file_type();

        if file_type.is_symlink() {
            let target = fs::read_link(entry.path())?;
            std::os::unix::fs::symlink(&target, &destination_path)?;
        } else if file_type.is_dir() {
            fs::create_dir_all(&destination_path)?;
        } else if file_type.is_file() {
            let mut source_file = File::open(entry.path())?;
            let mut dest_file = File::create(&destination_path)?;
            copy_with_progress(&mut source_file, &mut dest_file, progress_callback)?;
        }
    }

    Ok(())
}
