use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

pub fn create_spinner(message: String) -> anyhow::Result<ProgressBar> {
    let pb = ProgressBar::new_spinner();

    pb.set_message(message);
    pb.enable_steady_tick(Duration::from_millis(100));
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")?.tick_strings(&[
            "▹▹▹▹▹",
            "▸▹▹▹▹",
            "▹▸▹▹▹",
            "▹▹▸▹▹",
            "▹▹▹▸▹",
            "▹▹▹▹▸",
            "▪▪▪▪▪",
        ]),
    );

    Ok(pb)
}

pub fn create_progress_bar(total_size: u64) -> anyhow::Result<ProgressBar> {
    let pb = ProgressBar::new(total_size);

    pb.enable_steady_tick(Duration::from_millis(40));

    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")? 
            .progress_chars("#>-"),
    );

    Ok(pb)
}
