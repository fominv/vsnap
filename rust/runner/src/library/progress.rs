use std::{
    cmp::min,
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::Result;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf, Stdout, stdout},
    sync::mpsc::{Receiver, Sender},
};
use vsnap::library::Progress;

pub struct AsyncProgressReaderWriter<W> {
    inner: W,
    sender: Sender<u64>,
}

impl<W> AsyncProgressReaderWriter<W> {
    pub fn new(inner: W, sender: Sender<u64>) -> Self {
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

impl<W: AsyncRead + Unpin> AsyncRead for AsyncProgressReaderWriter<W> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let before = buf.filled().len();

        let inner = &mut self.inner;
        let poll = Pin::new(inner).poll_read(cx, buf);

        if let Poll::Ready(Ok(())) = &poll {
            let after = buf.filled().len();
            let bytes_read = after - before;

            let sender = self.sender.clone();

            tokio::spawn(async move {
                sender.send(bytes_read as u64).await.ok();
            });
        }

        poll
    }
}

pub struct ProgressReporter {
    stdout: Stdout,
    receiver: Receiver<u64>,
    total_size: u64,
    progress: u64,
}

impl ProgressReporter {
    pub fn new(receiver: Receiver<u64>, total_size: u64) -> Self {
        Self {
            stdout: stdout(),
            receiver,
            total_size,
            progress: 0,
        }
    }

    pub fn listen(mut self) {
        tokio::spawn(async move {
            while let Some(bytes_written) = self.receiver.recv().await {
                self.progress = min(self.progress + bytes_written, self.total_size);

                let mut output = match serde_json::to_vec(&Progress {
                    progress: self.progress,
                    total: self.total_size,
                }) {
                    Ok(output) => output,
                    Err(_) => {
                        continue;
                    }
                };

                output.push(b'\n');

                self.stdout.write_all(&output).await.ok();
            }
        });
    }
}
