pub struct Snapshot {
    pub timestamp: i64,
    pub name: String,
}

impl Snapshot {
    pub fn to_volume_name(&self) -> String {
        format!("vsnap-{}-{}", self.timestamp, self.name)
    }
}
