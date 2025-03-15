use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

pub static VERSION: LazyLock<String> =
    LazyLock::new(|| include_str!("../../../.version").trim().to_string());

#[derive(Serialize, Deserialize)]
pub struct ProgressStatus {
    pub progress: u64,
    pub total: u64,
}

#[derive(Serialize, Deserialize)]
pub enum ProgressLog {
    SnapshotProgress(ProgressStatus),
    RestoreProgress(ProgressStatus),
}
