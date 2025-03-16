use serde::{Deserialize, Serialize};

pub mod cli;
pub mod constant;
pub mod docker;
pub mod progress;
pub mod table;

#[derive(Serialize, Deserialize)]
pub struct Progress {
    pub progress: u64,
    pub total: u64,
}
