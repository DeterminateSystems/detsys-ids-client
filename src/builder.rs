use std::time::Duration;

use reqwest::Certificate;
use url::Url;

use crate::transport::TransportsError;
use crate::{system_snapshot::SystemSnapshotter, DeviceId, DistinctId, Map};
use crate::{Recorder, Worker};

#[derive(Default)]
pub struct Builder {
    distinct_id: Option<DistinctId>,
    device_id: Option<DeviceId>,
    endpoint: Option<String>,
    facts: Option<Map>,
    groups: Option<Map>,
    ssl_cert: Option<Certificate>,
    timeout: Option<Duration>,
    proxy: Option<Url>,
}

impl Builder {
    pub fn new() -> Self {
        Builder {
            distinct_id: None,
            device_id: None,
            endpoint: None,
            facts: None,
            groups: None,
            ssl_cert: None,
            timeout: None,
            proxy: None,
        }
    }

    pub fn set_distinct_id(&mut self, distinct_id: impl Into<DistinctId>) -> &mut Self {
        self.distinct_id = Some(distinct_id.into());
        self
    }

    pub fn set_device_id(&mut self, device_id: impl Into<DeviceId>) -> &mut Self {
        self.device_id = Some(device_id.into());
        self
    }

    pub fn set_facts(&mut self, facts: Map) -> &mut Self {
        self.facts = Some(facts);
        self
    }

    pub fn set_groups(&mut self, groups: Map) -> &mut Self {
        self.groups = Some(groups);
        self
    }

    pub fn add_fact(
        &mut self,
        key: impl Into<String> + std::fmt::Debug,
        value: impl Into<serde_json::Value>,
    ) -> &mut Self {
        self.facts
            .get_or_insert_with(Default::default)
            .insert(key.into(), value.into());
        self
    }

    pub fn set_endpoint(&mut self, endpoint: impl Into<String>) -> &mut Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    pub fn set_timeout(&mut self, duration: impl Into<Duration>) -> &mut Self {
        self.timeout = Some(duration.into());
        self
    }

    #[tracing::instrument(skip(self))]
    pub async fn try_set_ssl_cert_file(
        &mut self,
        ssl_cert_file: impl AsRef<std::path::Path> + std::fmt::Debug,
    ) -> Result<&mut Self, TransportsError> {
        self.ssl_cert = Some(read_cert_file(&ssl_cert_file).await?);
        Ok(self)
    }

    pub fn set_proxy(&mut self, proxy: Url) -> &mut Self {
        self.proxy = Some(proxy);
        self
    }

    #[tracing::instrument(skip(self))]
    pub async fn build(self) -> Result<(Recorder, Worker), TransportsError> {
        self.build_with_snapshotter(crate::system_snapshot::Generic::default())
            .await
    }

    #[tracing::instrument(skip(self, snapshotter))]
    pub async fn build_with_snapshotter<S: SystemSnapshotter>(
        mut self,
        snapshotter: S,
    ) -> Result<(Recorder, Worker), TransportsError> {
        let transport = crate::transport::Transports::try_new(
            self.endpoint.take(),
            self.timeout
                .take()
                .unwrap_or_else(|| Duration::from_secs(3)),
            self.ssl_cert.take(),
            self.proxy.take(),
        )
        .await?;

        let (recorder, worker) = Worker::new(
            self.distinct_id.take(),
            self.device_id.take(),
            self.facts.take(),
            self.groups.take(),
            snapshotter,
            transport,
        )
        .await;

        Ok((recorder, worker))
    }
}

#[tracing::instrument(ret(level = tracing::Level::TRACE))]
async fn read_cert_file(
    ssl_cert_file: impl AsRef<std::path::Path> + std::fmt::Debug,
) -> Result<Certificate, TransportsError> {
    let cert_buf = tokio::fs::read(&ssl_cert_file)
        .await
        .map_err(|e| TransportsError::Read(ssl_cert_file.as_ref().to_path_buf(), e))?;

    if let Ok(cert) = Certificate::from_pem(cert_buf.as_slice()) {
        return Ok(cert);
    }

    if let Ok(cert) = Certificate::from_der(cert_buf.as_slice()) {
        return Ok(cert);
    }

    Err(TransportsError::UnknownCertFormat)
}
