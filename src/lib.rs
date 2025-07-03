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

pub use builder::Builder;
pub use identity::{AnonymousDistinctId, DeviceId, DistinctId};
pub use recorder::Recorder;
pub use worker::Worker;

pub type Map = serde_json::Map<String, serde_json::Value>;

#[macro_export]
macro_rules! builder {
    () => {{
        detsys_ids_client::Builder::new()
            .fact("cargo_pkg_name", env!("CARGO_PKG_NAME"))
            .fact("$app_version", env!("CARGO_PKG_VERSION"))
            .fact("$app_name", env!("CARGO_CRATE_NAME"))
    }};
}
