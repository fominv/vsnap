use std::{fs::File, path::Path};

use anyhow::Result;
use tar::{Archive, Builder};
use zstd::Encoder;

use crate::library::constant::{SNAPSHOT_SUB_DIR, SNAPSHOT_TAR_ZST};

pub fn snapshot(source_path: &Path, snapshot_path: &Path, compress: bool) -> Result<()> {
    match compress {
        true => {
            let tar_file = File::create(&snapshot_path.join(SNAPSHOT_TAR_ZST))?;

            let mut encoder = Encoder::new(tar_file, 0)?.auto_finish();
            let mut tar_builder = Builder::new(&mut encoder);

            tar_builder.append_dir_all(".", &source_path)?;
        }
        false => {
            fs_extra::dir::create(snapshot_path.join(SNAPSHOT_SUB_DIR), false)?;

            fs_extra::dir::copy(
                source_path,
                snapshot_path.join(SNAPSHOT_SUB_DIR),
                &fs_extra::dir::CopyOptions {
                    content_only: true,
                    ..Default::default()
                },
            )?;
        }
    }

    Ok(())
}

pub fn restore(snapshot_path: &Path, restore_path: &Path) -> Result<()> {
    let is_compressed = snapshot_path.join(SNAPSHOT_TAR_ZST).exists();

    match is_compressed {
        true => {
            let snapshot_file = File::open(&snapshot_path.join(SNAPSHOT_TAR_ZST))?;

            let mut decoder = zstd::Decoder::new(snapshot_file)?;
            let mut archive = Archive::new(&mut decoder);

            archive.unpack(&restore_path)?;
        }
        false => {
            fs_extra::dir::copy(
                snapshot_path.join(SNAPSHOT_SUB_DIR),
                &restore_path,
                &fs_extra::dir::CopyOptions {
                    content_only: true,
                    ..Default::default()
                },
            )?;
        }
    }

    Ok(())
}
