use std::time::Duration;

use reqwest::Certificate;
use url::Url;

use crate::identity::AnonymousDistinctId;
use crate::storage::Storage;
use crate::transport::TransportsError;
use crate::{DeviceId, DistinctId, Map, system_snapshot::SystemSnapshotter};
use crate::{Groups, Recorder, Worker};

macro_rules! build_with_default_storage {
    ($self:expr, $transport:expr, $snapshot:expr) => {{
        match $crate::storage::JsonFile::try_default().await {
            Ok(json) => $self.build_with($transport, $snapshot, json).await,
            Err(e) => {
                tracing::debug!(
                    ?e,
                    "Failed to construct the default JsonFile storage, falling back to in-memory"
                );

                $self
                    .build_with($transport, $snapshot, $crate::storage::Generic::default())
                    .await
            }
        }
    }};
}

#[derive(Default, Clone)]
pub struct Builder {
    device_id: Option<DeviceId>,
    distinct_id: Option<DistinctId>,
    anonymous_distinct_id: Option<AnonymousDistinctId>,
    enable_reporting: bool,
    endpoint: Option<String>,
    facts: Option<Map>,
    groups: Option<Groups>,
    proxy: Option<Url>,
    certificate: Option<Certificate>,
    timeout: Option<Duration>,
}

impl Builder {
    pub fn new() -> Self {
        Builder {
            device_id: None,
            distinct_id: None,
            anonymous_distinct_id: None,
            enable_reporting: true,
            endpoint: None,
            facts: None,
            groups: None,
            proxy: None,
            certificate: None,
            timeout: None,
        }
    }

    pub fn anonymous_distinct_id(
        mut self,
        anonymous_distinct_id: Option<AnonymousDistinctId>,
    ) -> Self {
        self.set_anonymous_distinct_id(anonymous_distinct_id);
        self
    }

    pub fn set_anonymous_distinct_id(
        &mut self,
        anonymous_distinct_id: Option<AnonymousDistinctId>,
    ) -> &mut Self {
        self.anonymous_distinct_id = anonymous_distinct_id;
        self
    }

    pub fn distinct_id(mut self, distinct_id: Option<DistinctId>) -> Self {
        self.set_distinct_id(distinct_id);
        self
    }

    pub fn set_distinct_id(&mut self, distinct_id: Option<DistinctId>) -> &mut Self {
        self.distinct_id = distinct_id;
        self
    }

    pub fn device_id(mut self, device_id: Option<DeviceId>) -> Self {
        self.set_device_id(device_id);
        self
    }

    pub fn set_device_id(&mut self, device_id: Option<DeviceId>) -> &mut Self {
        self.device_id = device_id;
        self
    }

    pub fn facts(mut self, facts: Option<Map>) -> Self {
        self.set_facts(facts);
        self
    }

    pub fn set_facts(&mut self, facts: Option<Map>) -> &mut Self {
        self.facts = facts;
        self
    }

    pub fn groups(mut self, groups: Option<Groups>) -> Self {
        self.set_groups(groups);
        self
    }

    pub fn set_groups(&mut self, groups: Option<Groups>) -> &mut Self {
        self.groups = groups;
        self
    }

    pub fn fact(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.set_fact(key, value);
        self
    }

    pub fn set_fact(
        &mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> &mut Self {
        self.facts
            .get_or_insert_with(Default::default)
            .insert(key.into(), value.into());
        self
    }

    pub fn endpoint(mut self, endpoint: Option<String>) -> Self {
        self.set_endpoint(endpoint);
        self
    }

    pub fn set_endpoint(&mut self, endpoint: Option<String>) -> &mut Self {
        self.endpoint = endpoint;
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
    ///   .enable_reporting(!cli.no_telemetry)
    ///   .build_or_default()
    ///   .await;
    /// # })
    /// ```
    pub fn enable_reporting(mut self, enable_reporting: bool) -> Self {
        self.set_enable_reporting(enable_reporting);
        self
    }

    pub fn set_enable_reporting(&mut self, enable_reporting: bool) -> &mut Self {
        self.enable_reporting = enable_reporting;
        self
    }

    pub fn timeout(mut self, duration: Option<Duration>) -> Self {
        self.set_timeout(duration);
        self
    }

    pub fn set_timeout(&mut self, duration: Option<Duration>) -> &mut Self {
        self.timeout = duration;
        self
    }

    pub fn certificate(mut self, certificate: Option<Certificate>) -> Self {
        self.set_certificate(certificate);
        self
    }

    pub fn set_certificate(&mut self, certificate: Option<Certificate>) -> &mut Self {
        self.certificate = certificate;
        self
    }

    pub fn proxy(mut self, proxy: Option<Url>) -> Self {
        self.set_proxy(proxy);
        self
    }

    pub fn set_proxy(&mut self, proxy: Option<Url>) -> &mut Self {
        self.proxy = proxy;
        self
    }

    #[tracing::instrument(skip(self))]
    pub async fn try_build(mut self) -> Result<(Recorder, Worker), TransportsError> {
        let transport = self.transport().await?;

        Ok(build_with_default_storage!(
            self,
            transport,
            crate::system_snapshot::Generic::default()
        ))
    }

    #[tracing::instrument(skip(self))]
    pub async fn build_or_default(mut self) -> (Recorder, Worker) {
        let transport = self.transport_or_default().await;

        build_with_default_storage!(self, transport, crate::system_snapshot::Generic::default())
    }

    #[tracing::instrument(skip(self, snapshotter, storage))]
    pub async fn try_build_with<S: SystemSnapshotter, P: Storage>(
        mut self,
        snapshotter: S,
        storage: P,
    ) -> Result<(Recorder, Worker), TransportsError> {
        let transport = self.transport().await?;

        Ok(self.build_with(transport, snapshotter, storage).await)
    }

    #[tracing::instrument(skip(self, snapshotter, storage))]
    pub async fn build_or_default_with<S: SystemSnapshotter, P: Storage>(
        mut self,
        snapshotter: S,
        storage: P,
    ) -> (Recorder, Worker) {
        let transport = self.transport_or_default().await;

        self.build_with(transport, snapshotter, storage).await
    }

    #[tracing::instrument(skip(self, transport, snapshotter, storage))]
    async fn build_with<S: SystemSnapshotter, P: Storage>(
        &mut self,
        transport: crate::transport::Transports,
        snapshotter: S,
        storage: P,
    ) -> (Recorder, Worker) {
        Worker::new(
            self.anonymous_distinct_id.take(),
            self.distinct_id.take(),
            self.device_id.take(),
            self.facts.take(),
            self.groups.take(),
            snapshotter,
            storage,
            transport,
        )
        .await
    }

    async fn transport_or_default(&mut self) -> crate::transport::Transports {
        match self.transport().await {
            Ok(t) => {
                return t;
            }
            Err(e) => {
                tracing::warn!(%e, "Failed to construct the transport as configured, falling back to the default");
            }
        }

        match crate::transport::Transports::try_new(
            None,
            self.timeout
                .take()
                .unwrap_or_else(|| Duration::from_secs(3)),
            None,
            None,
        )
        .await
        {
            Ok(t) => {
                return t;
            }
            Err(e) => {
                tracing::warn!(%e, "Failed to construct the default transport, falling back to none");
            }
        }

        crate::transport::Transports::none()
    }

    async fn transport(&mut self) -> Result<crate::transport::Transports, TransportsError> {
        if self.enable_reporting {
            crate::transport::Transports::try_new(
                self.endpoint.take(),
                self.timeout.unwrap_or_else(|| Duration::from_secs(3)),
                self.certificate.take(),
                self.proxy.take(),
            )
            .await
        } else {
            Ok(crate::transport::Transports::none())
        }
    }
}
