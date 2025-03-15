use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

pub static VERSION: LazyLock<String> =
    LazyLock::new(|| include_str!("../../../.version").trim().to_string());

#[derive(Serialize, Deserialize)]
pub struct InProgress {
    progress: u64,
    total: u64,
}

#[derive(Serialize, Deserialize)]
pub enum ProgressStatus {
    InProgress(InProgress),
    Completed,
}

#[derive(Serialize, Deserialize)]
pub enum ProgressLog {
    SnapshotProgress(ProgressStatus),
    RestoreProgress(ProgressStatus),
}
