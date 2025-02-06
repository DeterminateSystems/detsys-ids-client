use crate::Map;

mod generic;
pub use generic::Generic;

#[derive(Clone, Debug, serde::Serialize)]
pub struct SystemSnapshot {
    /// Example: `grahams-macbook-pro.local`
    pub host_name: Option<String>,

    /// Example: `macOS Version 14.4.1 (Build 23E224)`
    #[serde(rename = "$os", skip_serializing_if = "Option::is_none")]
    pub operating_system: Option<String>,

    /// Example: `14.4.1`
    #[serde(rename = "$os_version", skip_serializing_if = "Option::is_none")]
    pub operating_system_version: Option<String>,

    #[serde(rename = "$locale", skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,

    #[serde(rename = "$timezone", skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,

    pub target_triple: String,
    pub stdin_is_terminal: bool,
    pub is_ci: bool,

    /// Example: `14`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processor_count: Option<u64>,

    /// Example: `38654705664`
    pub physical_memory_bytes: u64,

    /// Unix timestamp of the time the system booted. Example: `1713092739`
    pub boot_time: u64,

    /// Example: `determinate-nixd`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_name: Option<String>,

    /// Additional fields to be flattened into the snapshot data
    #[serde(flatten)]
    pub extra_fields: Option<Map>,
}

pub trait SystemSnapshotter: Send + Sync + 'static {
    fn snapshot(&self) -> SystemSnapshot;
}
