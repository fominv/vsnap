use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
pub struct Cli {
    #[clap(long, short)]
    pub source: PathBuf,

    #[clap(long, short)]
    pub target: PathBuf,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Snapshot { compression: bool },
    Restore {},
}
