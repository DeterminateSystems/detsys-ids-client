mod builder;
pub mod checkin;
mod collator;
mod compression_set;
mod configuration_proxy;
mod ds_correlation;
mod identity;
mod json_string;
mod recorder;
pub mod storage;
mod submitter;
pub mod system_snapshot;
pub mod transport;
mod worker;

use std::collections::HashMap;

pub use builder::Builder;
pub use identity::{AnonymousDistinctId, DeviceId, DistinctId};
pub use recorder::Recorder;
pub use worker::Worker;

pub type Map = serde_json::Map<String, serde_json::Value>;
pub type Groups = HashMap<String, String>;

#[macro_export]
macro_rules! builder {
    () => {{
        let builder = detsys_ids_client::Builder::new()
            .fact("cargo_pkg_name", env!("CARGO_PKG_NAME"))
            .fact("$app_version", env!("CARGO_PKG_VERSION"))
            .fact("$app_name", env!("CARGO_CRATE_NAME"))
            .enable_reporting(detsys_ids_client::is_telemetry_enabled())
            .endpoint(detsys_ids_client::get_ambient_transport_endpoint());

        builder
    }};
}

pub fn is_telemetry_enabled() -> bool {
    let enabled = std::env::var_os("DETSYS_IDS_TELEMETRY").as_deref()
        != Some(std::ffi::OsStr::new("disabled"));

    if !enabled {
        eprintln!(
            "{}",
            [
                "[NOTE] Telemetry is disabled, which makes it harder to build great software.",
                "We don't collect much, so please turn it back on:",
                "https://dtr.mn/telemetry",
            ]
            .join(" ")
        );
    }

    enabled
}

pub fn get_ambient_transport_endpoint() -> Option<String> {
    std::env::var("DETSYS_IDS_TRANSPORT").ok()
}
