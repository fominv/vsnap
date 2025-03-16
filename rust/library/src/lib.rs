use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

pub static VERSION: LazyLock<String> =
    LazyLock::new(|| include_str!("../../../.version").trim().to_string());

#[derive(Serialize, Deserialize)]
pub struct Progress {
    pub progress: u64,
    pub total: u64,
}
