mod library;

use library::cli::run;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run()
}
