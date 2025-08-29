mod generic;
mod json_file;

pub use generic::Generic;
pub use json_file::JsonFile;

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

pub enum DefaultStorageChain {
    JsonFile(JsonFile),
    Generic(Generic),
}

#[derive(thiserror::Error, Debug)]
pub enum DefaultStorageChainError {
    #[error(transparent)]
    JsonFile(#[from] <JsonFile as Storage>::Error),

    #[error(transparent)]
    Generic(#[from] <Generic as Storage>::Error),
}

impl DefaultStorageChain {
    pub async fn new() -> DefaultStorageChain {
        match JsonFile::try_default().await {
            Ok(json) => Self::JsonFile(json),
            Err(e) => {
                tracing::debug!(
                    ?e,
                    "Failed to construct the default JsonFile storage, falling back to in-memory"
                );
                Self::Generic(Generic::default())
            }
        }
    }
}

impl Storage for DefaultStorageChain {
    type Error = DefaultStorageChainError;

    async fn load(&self) -> Result<Option<StoredProperties>, Self::Error> {
        match self {
            DefaultStorageChain::JsonFile(json_file) => Ok(json_file.load().await?),
            DefaultStorageChain::Generic(generic) => Ok(generic.load().await?),
        }
    }

    async fn store(&mut self, properties: StoredProperties) -> Result<(), Self::Error> {
        match self {
            DefaultStorageChain::JsonFile(json_file) => Ok(json_file.store(properties).await?),
            DefaultStorageChain::Generic(generic) => Ok(generic.store(properties).await?),
        }
    }
}
