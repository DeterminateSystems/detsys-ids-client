use crate::system_snapshot::{SystemSnapshot, SystemSnapshotter};

#[derive(Default)]
pub struct Generic {}

impl SystemSnapshotter for Generic {
    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    fn snapshot(&self) -> SystemSnapshot {
        SystemSnapshot::default()
    }
}
