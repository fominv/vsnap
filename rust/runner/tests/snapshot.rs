use std::{fs, path::Path};

use anyhow::Result;
use rand::{Rng, SeedableRng, distr::Alphanumeric, rngs::StdRng};
use tempfile::tempdir;
use vsnap_runner::library::snapshot::{restore, snapshot};
use walkdir::WalkDir;

fn create_random_files(
    base_dir: &Path,
    num_files: usize,
    num_dirs: usize,
    rng: &mut StdRng,
) -> Result<()> {
    for _ in 0..num_dirs {
        let dir_name: String = rng
            .sample_iter(&Alphanumeric)
            .take(10)
            .map(char::from)
            .collect();
        let dir_path = base_dir.join(dir_name);
        fs::create_dir_all(&dir_path)?;

        for _ in 0..num_files {
            let file_name: String = rng
                .sample_iter(&Alphanumeric)
                .take(10)
                .map(char::from)
                .collect();

            let content: String = rng
                .sample_iter(&Alphanumeric)
                .take(100)
                .map(char::from)
                .collect();

            fs::write(dir_path.join(file_name), content)?;
        }
    }

    Ok(())
}

fn compare_directories(dir1: &Path, dir2: &Path) -> Result<()> {
    let entries1 = WalkDir::new(dir1)
        .sort_by_file_name()
        .into_iter()
        .collect::<Vec<_>>();
    let entries2 = WalkDir::new(dir2)
        .sort_by_file_name()
        .into_iter()
        .collect::<Vec<_>>();

    assert_eq!(
        entries1.len(),
        entries2.len(),
        "Directory entry count mismatch"
    );

    for (entry1, entry2) in entries1.into_iter().zip(entries2.into_iter()) {
        let entry1 = entry1?;
        let entry2 = entry2?;

        assert_eq!(
            entry1.file_type(),
            entry2.file_type(),
            "File type mismatch: {:?} vs {:?}",
            entry1.path(),
            entry2.path()
        );

        assert_eq!(
            entry1.path().strip_prefix(dir1)?,
            entry2.path().strip_prefix(dir2)?,
            "File path mismatch: {:?} vs {:?}",
            entry1.path(),
            entry2.path()
        );

        assert_eq!(
            entry1.metadata()?.permissions(),
            entry2.metadata()?.permissions(),
            "File permissions mismatch: {:?} vs {:?}",
            entry1.path(),
            entry2.path()
        );

        if entry1.file_type().is_file() {
            let content1 = fs::read(entry1.path())?;
            let content2 = fs::read(entry2.path())?;

            assert_eq!(
                content1,
                content2,
                "File content mismatch: {:?} vs {:?}",
                entry1.path(),
                entry2.path()
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_snapshot_restore_uncompressed() -> Result<()> {
    let source_dir = tempdir()?;
    let snapshot_dir = tempdir()?;
    let target_dir = tempdir()?;

    let mut rng = StdRng::seed_from_u64(0); // Seed for deterministic tests

    create_random_files(source_dir.path(), 5, 3, &mut rng)?;

    snapshot(source_dir.path(), snapshot_dir.path(), false).await?;
    restore(snapshot_dir.path(), target_dir.path()).await?;

    compare_directories(source_dir.path(), target_dir.path())?;

    Ok(())
}

#[tokio::test]
async fn test_snapshot_restore_compressed() -> Result<()> {
    let source_dir = tempdir()?;
    let snapshot_dir = tempdir()?;
    let target_dir = tempdir()?;

    let mut rng = StdRng::seed_from_u64(1); // Different seed for compressed to ensure variety

    create_random_files(source_dir.path(), 5, 3, &mut rng)?;

    snapshot(source_dir.path(), snapshot_dir.path(), true).await?;
    restore(snapshot_dir.path(), target_dir.path()).await?;

    compare_directories(source_dir.path(), target_dir.path())?;

    Ok(())
}

#[tokio::test]
async fn test_snapshot_restore_empty_dir_uncompressed() -> Result<()> {
    let source_dir = tempdir()?;
    let snapshot_dir = tempdir()?;
    let target_dir = tempdir()?;

    fs::create_dir_all(source_dir.path().join("empty_dir"))?;

    snapshot(source_dir.path(), snapshot_dir.path(), false).await?;
    restore(snapshot_dir.path(), target_dir.path()).await?;

    compare_directories(source_dir.path(), target_dir.path())?;

    Ok(())
}

#[tokio::test]
async fn test_snapshot_restore_empty_dir_compressed() -> Result<()> {
    let source_dir = tempdir()?;
    let snapshot_dir = tempdir()?;
    let target_dir = tempdir()?;

    fs::create_dir_all(source_dir.path().join("empty_dir"))?;

    snapshot(source_dir.path(), snapshot_dir.path(), true).await?;
    restore(snapshot_dir.path(), target_dir.path()).await?;

    compare_directories(source_dir.path(), target_dir.path())?;

    Ok(())
}

#[tokio::test]
async fn test_snapshot_restore_no_files_uncompressed() -> Result<()> {
    let source_dir = tempdir()?;
    let snapshot_dir = tempdir()?;
    let target_dir = tempdir()?;

    snapshot(source_dir.path(), snapshot_dir.path(), false).await?;
    restore(snapshot_dir.path(), target_dir.path()).await?;

    compare_directories(source_dir.path(), target_dir.path())?;

    Ok(())
}

#[tokio::test]
async fn test_snapshot_restore_no_files_compressed() -> Result<()> {
    let source_dir = tempdir()?;
    let snapshot_dir = tempdir()?;
    let target_dir = tempdir()?;

    snapshot(source_dir.path(), snapshot_dir.path(), true).await?;
    restore(snapshot_dir.path(), target_dir.path()).await?;

    compare_directories(source_dir.path(), target_dir.path())?;

    Ok(())
}
