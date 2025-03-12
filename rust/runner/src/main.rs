mod library;

use anyhow::Result;
use clap::Parser;
use library::cli::{Cli, Commands};
use std::fs::File;
use tar::{Archive, Builder};
use zstd::Encoder;

static SNAPSHOT_TAR_ZST: &str = "snapshot.tar.zst";

fn main() -> Result<()> {
    let args = Cli::parse();

    let source = args.source;
    let target = args.target;

    match args.command {
        Commands::Snapshot { compression } => match compression {
            true => {
                let snapshot_path = target.join(SNAPSHOT_TAR_ZST);
                let tar_file = File::create(&snapshot_path)?;

                let mut encoder = Encoder::new(tar_file, 0)?.auto_finish();
                let mut tar_builder = Builder::new(&mut encoder);

                tar_builder.append_dir_all(".", &source)?;
            }
            false => {
                fs_extra::dir::create(target.join("files"), false)?;

                fs_extra::dir::copy(
                    source,
                    target.join("files"),
                    &fs_extra::dir::CopyOptions {
                        content_only: true,
                        ..Default::default()
                    },
                )?;
            }
        },

        Commands::Restore {} => {
            let is_compressed = target.join(SNAPSHOT_TAR_ZST).exists();

            match is_compressed {
                true => {
                    let snapshot_path = source.join(SNAPSHOT_TAR_ZST);
                    let snapshot_file = File::open(&snapshot_path)?;

                    let mut decoder = zstd::Decoder::new(snapshot_file)?;
                    let mut archive = Archive::new(&mut decoder);

                    archive.unpack(&target)?;
                }
                false => {
                    fs_extra::dir::copy(
                        source.join("files"),
                        &target,
                        &fs_extra::dir::CopyOptions {
                            content_only: true,
                            ..Default::default()
                        },
                    )?;
                }
            }
        }
    }

    Ok(())
}
