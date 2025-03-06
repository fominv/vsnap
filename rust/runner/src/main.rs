mod library;

use std::fs::File;

use anyhow::Result;
use clap::Parser;
use tar::{Archive, Builder};
use volsnap_library::cli::{Cli, Commands};
use zstd::Encoder;

fn main() -> Result<()> {
    let args = Cli::parse();

    let source = args.source;
    let target = args.target;

    match args.command {
        Commands::Snapshot { compression } => match compression {
            true => {
                let snapshot_path = target.join("snapshot.tar.zst");
                let tar_file = File::create(&snapshot_path)?;

                let mut encoder = Encoder::new(tar_file, 0)?.auto_finish();
                let mut tar_builder = Builder::new(&mut encoder);

                tar_builder.append_dir_all(".", &source)?;
            }
            false => {
                fs_extra::dir::copy(
                    source,
                    target.join("files"),
                    &fs_extra::dir::CopyOptions::default(),
                )?;
            }
        },

        Commands::Restore {} => {
            let is_compressed = target.join("snapshot.tar.zst").exists();

            match is_compressed {
                true => {
                    let snapshot_path = source.join("snapshot.tar.zst");
                    let snapshot_file = File::open(&snapshot_path)?;
                    let mut decoder = zstd::Decoder::new(snapshot_file)?;
                    let mut archive = Archive::new(&mut decoder);
                    archive.unpack(&target)?;
                }
                false => {
                    fs_extra::dir::copy(
                        source.join("files"),
                        &target,
                        &fs_extra::dir::CopyOptions::default(),
                    )?;
                }
            }
        }
    }

    Ok(())
}
