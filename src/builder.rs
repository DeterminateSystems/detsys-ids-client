use std::time::Duration;

use reqwest::Certificate;
use url::Url;

use crate::transport::TransportsError;
use crate::{system_snapshot::SystemSnapshotter, DeviceId, DistinctId, Map};
use crate::{Recorder, Worker};

#[derive(Default)]
pub struct Builder {
    device_id: Option<DeviceId>,
    distinct_id: Option<DistinctId>,
    enable_reporting: bool,
    endpoint: Option<String>,
    facts: Option<Map>,
    groups: Option<Map>,
    proxy: Option<Url>,
    ssl_cert: Option<Certificate>,
    timeout: Option<Duration>,
}

impl Builder {
    pub fn new() -> Self {
        Builder {
            device_id: None,
            distinct_id: None,
            enable_reporting: true,
            endpoint: None,
            facts: None,
            groups: None,
            proxy: None,
            ssl_cert: None,
            timeout: None,
        }
    }

    pub fn set_distinct_id(mut self, distinct_id: impl Into<DistinctId>) -> Self {
        self.distinct_id = Some(distinct_id.into());
        self
    }

    pub fn set_device_id(mut self, device_id: impl Into<DeviceId>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    pub fn set_facts(mut self, facts: Map) -> Self {
        self.facts = Some(facts);
        self
    }

    pub fn set_groups(mut self, groups: Map) -> Self {
        self.groups = Some(groups);
        self
    }

    pub fn add_fact(
        mut self,
        key: impl Into<String> + std::fmt::Debug,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.facts
            .get_or_insert_with(Default::default)
            .insert(key.into(), value.into());
        self
    }

    pub fn set_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set whether reporting is enabled or disabled.
    /// Reporting is enabled by default, but this function can be used in a pipeline for easy configuration:
    ///
    /// ```rust
    /// use detsys_ids_client::builder;
    ///
    /// struct Cli {
    ///   no_telemetry: bool,
    /// }
    ///
    ///
    /// # tokio_test::block_on(async {
    ///
    /// let cli = Cli { no_telemetry: false, };
    ///
    /// let (recorder, worker) = builder!()
    ///   .set_enable_reporting(!cli.no_telemetry)
    ///   .build()
    ///   .await
    ///   .unwrap();
    /// # })
    /// ```
    pub fn set_enable_reporting(mut self, enable_reporting: bool) -> Self {
        self.enable_reporting = enable_reporting;
        self
    }

    pub fn set_timeout(mut self, duration: impl Into<Duration>) -> Self {
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

    pub fn set_proxy(mut self, proxy: Url) -> Self {
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
        let transport = if self.enable_reporting {
            crate::transport::Transports::try_new(
                self.endpoint.take(),
                self.timeout
                    .take()
                    .unwrap_or_else(|| Duration::from_secs(3)),
                self.ssl_cert.take(),
                self.proxy.take(),
            )
            .await?
        } else {
            crate::transport::Transports::none()
        };

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
