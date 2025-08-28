mod generic;

pub use generic::Generic;

use crate::checkin::Checkin;
use crate::identity::AnonymousDistinctId;
use crate::{DeviceId, DistinctId, Groups};

#[derive(Default, Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredProperties {
    pub anonymous_distinct_id: AnonymousDistinctId,
    pub distinct_id: Option<DistinctId>,
    pub device_id: DeviceId,
    #[serde(default)]
    pub groups: Groups,
    #[serde(default)]
    pub checkin: Checkin,
}

pub trait Storage: Send + Sync + 'static {
    type Error: std::fmt::Debug + std::fmt::Display;

    fn load(
        &self,
    ) -> impl std::future::Future<Output = Result<Option<StoredProperties>, Self::Error>> + Send;
    fn store(
        &mut self,
        properties: StoredProperties,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;
}
