use std::io::Write;
use std::path::PathBuf;

use crate::storage::{Storage, StoredProperties};
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;

const XDG_PREFIX: &str = "systems.determinate.detsys-ids-client";
const XDG_STORAGE_FILENAME: &str = "storage.json";
const NOTES: &[&str] = &[
    "The IDs in this file are randomly generated UUIDs.",
    "Determinate Systems uses these IDs to know how many people use our software and how to focus our limited resources for research and development.",
    "The data here contains no personally identifiable information.",
    "You can delete this file at any time to create new IDs.",
    "",
    "See our privacy policy: https://determinate.systems/policies/privacy",
    "See our docs on telemetry: https://dtr.mn/telemetry",
];

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("No HOME is available")]
    NoHome,

    #[error("The storage location has no parent directory")]
    LocationHasNoParent,

    #[error("Serializing / deserializing failure: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Loading from storage failed when opening the file `{0}`: {1}")]
    Open(PathBuf, std::io::Error),

    #[error("Creating the storage file `{0}` failed: {1}")]
    Create(PathBuf, std::io::Error),

    #[error("Reading from storage at `{0}` failed: {1}")]
    Read(PathBuf, std::io::Error),

    #[error("Writing storage to `{0}` failed: {1}")]
    Write(PathBuf, std::io::Error),

    #[error(transparent)]
    Persist(#[from] tempfile::PersistError),

    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct WrappedStorage {
    notes: Vec<String>,
    body: StoredProperties,
}

pub struct JsonFile {
    location: PathBuf,
    directory: PathBuf,
}

impl JsonFile {
    #[allow(dead_code)]
    #[tracing::instrument]
    pub fn new(location: PathBuf) -> Option<Self> {
        Some(Self {
            directory: location.parent()?.to_owned(),
            location,
        })
    }

    pub async fn try_default() -> Result<Self, Error> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix(XDG_PREFIX);

        let file = xdg_dirs
            .place_state_file(XDG_STORAGE_FILENAME)
            .map_err(|e| {
                match xdg_dirs
                    .get_state_file(XDG_STORAGE_FILENAME)
                    .ok_or(Error::NoHome)
                {
                    Ok(loc) => Error::Create(loc, e),
                    Err(e) => e,
                }
            })?;

        Self::new(file).ok_or(Error::LocationHasNoParent)
    }
}

impl Storage for JsonFile {
    type Error = Error;

    #[tracing::instrument(skip(self))]
    async fn load(&self) -> Result<Option<StoredProperties>, Error> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(false)
            .create(false)
            .truncate(false)
            .open(&self.location)
            .await
            .map_err(|e| Error::Open(self.location.clone(), e))?;

        let mut contents = vec![];
        file.read_to_end(&mut contents)
            .await
            .map_err(|e| Error::Read(self.location.clone(), e))?;

        let wrapped: WrappedStorage = serde_json::from_slice(&contents)?;

        Ok(Some(wrapped.body))
    }

    #[tracing::instrument(skip(self, props))]
    async fn store(&mut self, props: StoredProperties) -> Result<(), Error> {
        let wrapped = WrappedStorage {
            notes: NOTES.iter().map(|v| String::from(*v)).collect(),
            body: props,
        };
        let json = serde_json::to_string_pretty(&wrapped)?;

        let directory = self.directory.clone();
        let location = self.location.clone();

        tracing::trace!("Storing properties");
        tokio::task::spawn_blocking(move || -> Result<(), Error> {
            let mut tempfile = tempfile::NamedTempFile::new_in(&directory)
                .map_err(|e| Error::Create(directory.clone(), e))?;

            tempfile
                .write_all(json.as_bytes())
                .map_err(|e| Error::Write(tempfile.path().into(), e))?;

            tempfile.persist(&location)?;

            Ok(())
        })
        .await??;

        tracing::trace!(location = ?self.location, "Storage persisted");

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::{
        AnonymousDistinctId,
        storage::{Storage, StoredProperties},
    };

    #[tokio::test]
    async fn round_trips() {
        let tempfile = tempfile::NamedTempFile::new().unwrap();

        let mut store = super::JsonFile::new(tempfile.path().into()).unwrap();
        let identity = StoredProperties {
            anonymous_distinct_id: AnonymousDistinctId::default(),
            device_id: "hi".to_string().into(),
            ..Default::default()
        };

        store.store(identity.clone()).await.unwrap();

        assert_eq!(identity, store.load().await.unwrap().unwrap());
    }
}
