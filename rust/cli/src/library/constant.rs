use std::sync::LazyLock;

use regex::Regex;

pub static VERSION: LazyLock<String> = LazyLock::new(|| env!("CARGO_PKG_VERSION").to_string());

pub static SNAPSHOT_PREFIX_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^vsnap-(\d{10,})-").expect("Failed to compile snapshot prefix regex")
});
