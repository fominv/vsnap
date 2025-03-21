mod library;

use crate::library::cli::run;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run().await
}
