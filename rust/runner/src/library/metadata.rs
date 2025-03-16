use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub total_size: u64,
}

impl SnapshotMetadata {
    pub fn new(total_size: u64) -> Self {
        SnapshotMetadata { total_size }
    }

    pub fn write(&self, path: &std::path::Path) -> anyhow::Result<()> {
        fs::write(path, serde_json::to_string(self)?)?;

        Ok(())
    }

    pub fn read(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)?;

        Ok(serde_json::from_str(&content)?)
    }
}
