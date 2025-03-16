use std::{
    cmp::min,
    path::{Path, PathBuf},
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::Result;
use async_compression::{
    Level,
    tokio::{bufread::ZstdDecoder, write::ZstdEncoder},
};
use tokio::{
    fs::File,
    io::{
        self, AsyncRead, AsyncWrite, AsyncWriteExt, BufStream, BufWriter, ReadBuf, Stdout, stdout,
    },
    sync::mpsc::{self, Receiver, Sender},
};
use tokio_tar::Builder;
use vsnap::library::Progress;

use crate::library::{
    constant::{SNAPSHOT_METADATA, SNAPSHOT_TAR, SNAPSHOT_TAR_ZST},
    metadata::SnapshotMetadata,
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
    let progress_writer = AsyncProgressReaderWriter::new(encoder, sender);

    let mut archive = Builder::new(progress_writer);
    archive.append_dir_all(dir_to_compress, ".").await?;
    archive.finish().await?;

    Ok(())
}

async fn decompress_dir(
    tar_file: tokio::fs::File,
    restore_path: &Path,
    sender: Sender<u64>,
) -> anyhow::Result<()> {
    Ok(())
}

async fn tar_dir(dir: &Path, tar_file: tokio::fs::File, sender: Sender<u64>) -> anyhow::Result<()> {
    let buffered_stream = BufStream::new(tar_file);
    let progress_writer = AsyncProgressReaderWriter::new(buffered_stream, sender);

    let mut archive = Builder::new(progress_writer);
    archive.append_dir_all(dir, ".").await?;
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

    let progress_writer = AsyncProgressReaderWriter::new(decoder, sender);

    let mut archive = tokio_tar::Archive::new(progress_writer);

    archive.unpack(restore_path).await?;

    Ok(())
}

struct AsyncProgressReaderWriter<W> {
    inner: W,
    sender: Sender<u64>,
}

impl<W> AsyncProgressReaderWriter<W> {
    fn new(inner: W, sender: Sender<u64>) -> Self {
        Self { inner, sender }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for AsyncProgressReaderWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let inner = &mut self.inner;
        let poll = Pin::new(inner).poll_write(cx, buf);

        if let Poll::Ready(Ok(bytes_written)) = poll {
            let sender = self.sender.clone();

            tokio::spawn(async move {
                sender.send(bytes_written as u64).await.ok();
            });
        }

        poll
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for AsyncProgressReaderWriter<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let inner = &mut self.inner;
        let initial_len = buf.filled().len();
        let poll = Pin::new(inner).poll_read(cx, buf);

        if let Poll::Ready(Ok(_)) = poll {
            let bytes_read = buf.filled().len() - initial_len;
            if bytes_read > 0 {
                let sender = self.sender.clone();
                tokio::spawn(async move {
                    sender.send(bytes_read as u64).await.ok();
                });
            }
            Poll::Ready(Ok(()))
        } else {
            poll
        }
    }
}

struct ProgressReporter {
    stdout_handle: BufWriter<Stdout>,
    receiver: Receiver<u64>,
    total_size: u64,
    progress: u64,
}

impl ProgressReporter {
    fn new(receiver: Receiver<u64>, total_size: u64) -> Self {
        let stdout_handle = BufWriter::new(stdout());

        Self {
            stdout_handle,
            receiver,
            total_size,
            progress: 0,
        }
    }

    fn listen(mut self) {
        tokio::spawn(async move {
            while let Some(bytes_written) = self.receiver.recv().await {
                self.progress = min(self.progress + bytes_written, self.total_size);

                serde_json::to_vec(&Progress {
                    progress: self.progress,
                    total: self.total_size,
                })
                .ok()
                .map(async |x| self.stdout_handle.write_all(x.as_ref()).await);
            }
        });
    }
}
