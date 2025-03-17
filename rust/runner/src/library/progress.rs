use std::{
    cmp::min,
    io::{self, BufWriter, Read, Write, stdout},
    sync::mpsc::{Receiver, Sender},
    thread,
};

use vsnap::library::Progress;

pub struct ProgressReporterWriter<W: Write> {
    inner: W,
    sender: Sender<u64>,
}

impl<W: Write> ProgressReporterWriter<W> {
    pub fn new(inner: W, sender: Sender<u64>) -> Self {
        ProgressReporterWriter { inner, sender }
    }
}

impl<W: Write> Write for ProgressReporterWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let bytes_written = self.inner.write(buf)?;

        self.sender.send(bytes_written as u64).ok();

        Ok(bytes_written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub struct ProgressReporterReader<R: Read> {
    inner: R,
    sender: Sender<u64>,
}

impl<R: Read> ProgressReporterReader<R> {
    pub fn new(inner: R, sender: Sender<u64>) -> Self {
        ProgressReporterReader { inner, sender }
    }
}

impl<R: Read> Read for ProgressReporterReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.inner.read(buf)?;

        self.sender.send(bytes_read as u64).ok();

        Ok(bytes_read)
    }
}

pub struct ProgressListener {
    stdout: BufWriter<io::Stdout>,
    total_size: u64,
    progress: u64,
    receiver: Receiver<u64>,
}

impl ProgressListener {
    pub fn new(total_size: u64, receiver: Receiver<u64>) -> Self {
        ProgressListener {
            stdout: BufWriter::new(stdout()),
            total_size,
            progress: 0,
            receiver,
        }
    }

    pub fn listen(mut self) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            loop {
                match self.receiver.recv() {
                    Ok(bytes_read) => {
                        self.progress = min(self.progress + bytes_read, self.total_size);

                        serde_json::to_string(&Progress {
                            progress: self.progress,
                            total: self.total_size,
                        })
                        .ok()
                        .map(|x| writeln!(self.stdout, "{}", x).ok());
                    }
                    Err(_) => break,
                }
            }
        })
    }
}
