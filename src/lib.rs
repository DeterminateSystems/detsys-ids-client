mod builder;
pub mod checkin;
mod collator;
mod configuration_proxy;
mod ds_correlation;
mod identity;
mod json_string;
mod recorder;
mod submitter;
pub mod system_snapshot;
pub mod transport;
mod worker;

pub use builder::Builder;
pub use identity::{DeviceId, DistinctId};
pub use recorder::Recorder;
pub use worker::Worker;

pub type Map = serde_json::Map<String, serde_json::Value>;

#[macro_export]
macro_rules! builder {
    () => {{
        let mut builder = Builder::new();
        builder.add_fact("cargo_pkg_name", env!("CARGO_PKG_NAME"));
        builder.add_fact("$app_version", env!("CARGO_PKG_VERSION"));
        builder.add_fact("$app_name", env!("CARGO_CRATE_NAME"));
        builder
    }};
}
