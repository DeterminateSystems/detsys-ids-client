use std::io::IsTerminal;

use crate::system_snapshot::{SystemSnapshot, SystemSnapshotter};

#[derive(Default)]
pub struct Generic {}

impl SystemSnapshotter for Generic {
    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    fn snapshot(&self) -> SystemSnapshot {
        let system = sysinfo::System::new_all();

        let is_ci = is_ci::cached()
            || std::env::var("DETSYS_IDS_IN_CI").unwrap_or_else(|_| "0".into()) == "1";

        SystemSnapshot {
            locale: sys_locale::get_locale(),
            timezone: iana_time_zone::get_timezone().ok(),

            host_name: sysinfo::System::host_name(),
            operating_system: sysinfo::System::long_os_version(),
            operating_system_version: sysinfo::System::os_version(),

            target_triple: target_lexicon::HOST.to_string(),
            stdin_is_terminal: std::io::stdin().is_terminal(),
            is_ci,

            processor_count: system.physical_core_count().map(
                |count| count as u64, /* safety: `as` truncates on overflow */
            ),
            physical_memory_bytes: system.total_memory(),
            boot_time: sysinfo::System::boot_time(),
            process_name: std::env::args().next(),

            extra_fields: None,
        }
    }
}
