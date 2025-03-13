use std::sync::LazyLock;

pub static VERSION: LazyLock<String> =
    LazyLock::new(|| include_str!("../../../.version").trim().to_string());
