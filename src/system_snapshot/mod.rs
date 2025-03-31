use std::io::IsTerminal;

use sysinfo::System;

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

impl Default for SystemSnapshot {
    fn default() -> Self {
        let system = System::new_all();

        let is_ci = is_ci::cached()
            || std::env::var("DETSYS_IDS_IN_CI").unwrap_or_else(|_| "0".into()) == "1";

        Self {
            locale: sys_locale::get_locale(),
            timezone: iana_time_zone::get_timezone().ok(),

            host_name: System::host_name(),
            operating_system: System::long_os_version(),
            operating_system_version: System::os_version(),

            target_triple: target_lexicon::HOST.to_string(),
            stdin_is_terminal: std::io::stdin().is_terminal(),
            is_ci,

            processor_count: System::physical_core_count().map(
                |count| count as u64, /* safety: `as` truncates on overflow */
            ),
            physical_memory_bytes: system.total_memory(),
            boot_time: System::boot_time(),
            process_name: std::env::args().next(),

            extra_fields: None,
        }
    }
}

pub trait SystemSnapshotter: Send + Sync + 'static {
    fn snapshot(&self) -> SystemSnapshot;
}
