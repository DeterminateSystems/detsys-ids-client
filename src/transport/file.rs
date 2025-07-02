use std::path::PathBuf;
use std::sync::Arc;

use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufWriter};
use tokio::sync::Mutex;

use crate::Map;
use crate::submitter::Batch;

use super::Transport;

#[derive(Clone)]
pub(crate) struct FileTransport {
    checkin: Option<(PathBuf, Arc<Mutex<File>>)>,

    output_path: PathBuf,
    output_handle: Arc<Mutex<BufWriter<File>>>,
}
impl FileTransport {
    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(err))]
    pub(crate) async fn new(
        output_path: impl Into<PathBuf> + std::fmt::Debug,
        checkin_path: Option<impl Into<PathBuf> + std::fmt::Debug>,
    ) -> Result<Self, <Self as Transport>::Error> {
        let output_path = output_path.into();
        let checkin_path = checkin_path.map(|e| e.into());

        let output_handle = File::create(&output_path)
            .await
            .map_err(|e| FileTransportError::FileOpen(output_path.clone(), e))
            .map(|f| Arc::new(Mutex::new(BufWriter::new(f))))?;

        let checkin = if let Some(checkin_path) = checkin_path {
            let handle = File::open(&checkin_path)
                .await
                .map_err(|e| FileTransportError::FileOpen(checkin_path.clone(), e))
                .map(|f| Arc::new(Mutex::new(f)))?;

            Some((checkin_path, handle))
        } else {
            None
        };

        Ok(FileTransport {
            checkin,
            output_path,
            output_handle,
        })
    }
}

impl Transport for FileTransport {
    type Error = FileTransportError;

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all))]
    async fn submit(&mut self, batch: Batch<'_>) -> Result<(), Self::Error> {
        let mut handle = self.output_handle.lock().await;

        handle
            .write_all(&serde_json::to_vec(&batch)?)
            .await
            .map_err(|e| FileTransportError::Write(self.output_path.clone(), e))?;
        handle
            .write(b"\n")
            .await
            .map_err(|e| FileTransportError::Write(self.output_path.clone(), e))?;
        handle
            .flush()
            .await
            .map_err(|e| FileTransportError::Flush(self.output_path.clone(), e))?;

        Ok(())
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all, ret(level = tracing::Level::TRACE)))]
    async fn checkin(
        &self,
        _session_properties: Map,
    ) -> Result<crate::checkin::Checkin, Self::Error> {
        let Some((path, handle)) = &self.checkin else {
            return Err(FileTransportError::NoConfiguration);
        };

        let mut handle = handle.lock().await;

        if let Err(e) = handle.seek(std::io::SeekFrom::Start(0)).await {
            tracing::debug!(%e, "Resetting to the beginning of the checkin file stream failed");
        }

        let mut buffer = Vec::new();
        handle
            .read_to_end(&mut buffer)
            .await
            .map_err(|e| FileTransportError::Read(path.clone(), e))?;

        Ok(serde_json::from_slice(&buffer)?)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum FileTransportError {
    #[error("Failure opening file '{0}': {1}")]
    FileOpen(PathBuf, std::io::Error),

    #[error("No configuration.")]
    NoConfiguration,

    #[error("Failure writing to the IDS diagnostics log at '{0}': {1}")]
    Write(PathBuf, std::io::Error),

    #[error("Failure flushing the IDS diagnostics log at '{0}': {1}")]
    Flush(PathBuf, std::io::Error),

    #[error("Failure reading the IDS diagnostics log at '{0}': {1}")]
    Read(PathBuf, std::io::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}
