use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

pub static VERSION: LazyLock<String> = LazyLock::new(|| env!("CARGO_PKG_VERSION").to_string());

#[derive(Serialize, Deserialize)]
pub struct Progress {
    pub progress: u64,
    pub total: u64,
}
