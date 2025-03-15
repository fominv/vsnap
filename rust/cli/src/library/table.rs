use std::collections::HashMap;

use console::style;
use tabled::{
    builder::Builder,
    settings::{Style, Theme},
};

use crate::library::docker::{VolumeSize, extract_snapshot_datetime, strip_snapshot_prefix};

pub fn print_snapshot_table(
    snapshot_volume_names: Vec<String>,
    volume_sizes: Option<HashMap<String, VolumeSize>>,
) -> anyhow::Result<()> {
    let mut header = vec!["Snapshot Name", "Local Datetime"];

    if volume_sizes.is_some() {
        header.push("Size");
    }

    header.push("Volume Name");

    let header = header
        .iter()
        .map(|s| style(s).green().bold().to_string())
        .collect::<Vec<String>>();

    let mut builder = Builder::default();
    builder.push_record(header);

    for snapshot_volume_name in snapshot_volume_names {
        let snapshot_name = strip_snapshot_prefix(&snapshot_volume_name);
        let local_datetime = extract_snapshot_datetime(&snapshot_volume_name)?;

        let mut record: Vec<String> = vec![snapshot_name, local_datetime.to_string()];

        if let Some(volume_size) = &volume_sizes {
            let size = match volume_size.get(&snapshot_volume_name) {
                Some(VolumeSize::Bytes(size)) => (size / 1024 / 1024).to_string() + " MB",
                _ => "Unavailable".to_string(),
            };

            record.push(size);
        }

        record.push(snapshot_volume_name);
        builder.push_record(record);
    }

    let mut table = builder.build();

    let mut style = Theme::from_style(Style::markdown());
    style.remove_borders_horizontal();

    table.with(style);

    println!("{}", table.to_string());

    Ok(())
}
