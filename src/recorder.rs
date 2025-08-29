use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot::channel as oneshot;
use tracing::Instrument;

use crate::checkin::{Checkin, Feature};
use crate::collator::FeatureFacts;
use crate::configuration_proxy::{CheckinStatus, ConfigurationProxySignal};
use crate::identity::DistinctId;
use crate::{Map, PersonProperties};

#[derive(Debug)]
pub(crate) enum RawSignal {
    Fact {
        key: String,
        value: serde_json::Value,
    },
    UpdateFeatureConfiguration(Option<Checkin>, FeatureFacts),
    Event {
        event_name: String,
        properties: Option<Map>,
    },
    GetSessionProperties {
        tx: tokio::sync::oneshot::Sender<Map>,
    },
    FlushNow,
    Identify(DistinctId, IdentifyProperties),
    SetPersonProperties(IdentifyProperties),
    AddGroup {
        group_name: String,
        group_member_id: String,
    },
    Alias(String),
    Reset,
}

#[derive(Default, Debug, serde::Serialize)]
pub struct IdentifyProperties {
    #[serde(rename = "$set")]
    pub set: PersonProperties,
    #[serde(rename = "$set_once")]
    pub set_once: PersonProperties,
}

impl IdentifyProperties {
    pub(crate) fn as_map(&self) -> Map {
        let val = serde_json::to_value(self)
            .inspect_err(|e| {
                tracing::error!(
                    self = ?&self,
                    error = ?e,
                    "IdentifyProperties cannot convert to a Map"
                );
            })
            .unwrap_or_default();

        let serde_json::Value::Object(map) = val else {
            tracing::error!(
                self = ?&self,
                "IdentifyProperties did not serialize to an Object"
            );
            return Map::default();
        };
        map
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RecorderError {
    #[error("Timed out waiting for configuration to complete: {0:?}")]
    WaitForConfiguration(#[from] tokio::time::error::Elapsed),

    #[error("Failed to subscribe to the ConfigurationProxy for configuration changes")]
    SubscribeFailed,

    #[error(transparent)]
    Subscription(#[from] tokio::sync::broadcast::error::RecvError),

    #[error("Failed to signal the configuration proxy: '{0}'")]
    SendToConfigurationProxy(String),

    #[error(transparent)]
    Response(#[from] tokio::sync::oneshot::error::RecvError),
}

pub struct Recorder {
    outgoing: Sender<RawSignal>,
    auto_refresh_config: bool,
    to_configuration_proxy: Sender<ConfigurationProxySignal>,
}

impl Clone for Recorder {
    fn clone(&self) -> Self {
        Self {
            outgoing: self.outgoing.clone(),
            auto_refresh_config: true,
            to_configuration_proxy: self.to_configuration_proxy.clone(),
        }
    }
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorder").finish()
    }
}

impl Recorder {
    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all))]
    pub(crate) fn new(
        snapshotter_tx: Sender<RawSignal>,
        to_configuration_proxy: Sender<ConfigurationProxySignal>,
    ) -> Self {
        Self {
            outgoing: snapshotter_tx,
            to_configuration_proxy,
            auto_refresh_config: true,
        }
    }

    // Execute a series of operations without triggering multiple configuration refreshes.
    // Note: there are no atomic semantics, and configuration is refreshed at the end no matter what your function does.
    pub async fn in_configuration_txn<F, T>(&self, f: F) -> T
    where
        F: AsyncFnOnce(&Recorder) -> T,
    {
        let mut rec = self.clone();

        rec.auto_refresh_config = false;

        let ret = f(self).await;

        rec.auto_refresh_config = true;
        rec.trigger_configuration_refresh().await;

        ret
    }

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature_variant<
        T: serde::ser::Serialize + serde::de::DeserializeOwned + std::fmt::Debug + Send,
    >(
        &self,
        key: impl Into<String> + std::fmt::Debug,
    ) -> Option<T> {
        serde_json::from_value(self.get_feature::<T>(key).await?.variant)
            .inspect_err(|e| tracing::debug!(%e, "Deserializing feature variant failed"))
            .ok()
    }

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature_ptr_variant<
        T: serde::ser::Serialize + serde::de::DeserializeOwned + Send + std::fmt::Debug,
    >(
        &self,
        key: impl Into<String> + std::fmt::Debug,
    ) -> Option<T> {
        serde_json::from_value(self.get_feature_ptr::<T>(key).await?.variant)
            .inspect_err(|e| tracing::debug!(%e, "Deserializing feature variant failed"))
            .ok()
    }

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature_payload<
        T: serde::ser::Serialize + serde::de::DeserializeOwned + Send + std::fmt::Debug,
    >(
        &self,
        key: impl Into<String> + std::fmt::Debug,
    ) -> Option<T> {
        self.get_feature::<T>(key).await?.payload
    }

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature_ptr_payload<
        T: serde::ser::Serialize + serde::de::DeserializeOwned + Send + std::fmt::Debug,
    >(
        &self,
        key: impl Into<String> + std::fmt::Debug,
    ) -> Option<T> {
        self.get_feature_ptr::<T>(key).await?.payload
    }

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature_ptr<
        T: serde::ser::Serialize + serde::de::DeserializeOwned + Send + std::fmt::Debug,
    >(
        &self,
        key: impl Into<String> + std::fmt::Debug,
    ) -> Option<Feature<T>> {
        let ptr = self.get_feature_payload::<String>(key).await?;
        self.get_feature::<T>(ptr).await
    }

    pub async fn wait_for_checkin(
        &self,
        duration: Option<std::time::Duration>,
    ) -> Result<(), RecorderError> {
        let (tx, rx) = oneshot();

        let subscription = self.subscribe_to_feature_changes().await;

        self.to_configuration_proxy
            .send(ConfigurationProxySignal::QueryIfCheckedIn(tx))
            .instrument(tracing::trace_span!(
                "requesting check in status from the configuration proxy"
            ))
            .await
            .map_err(|e| RecorderError::SendToConfigurationProxy(format!("{e:?}")))?;

        if rx.await? == CheckinStatus::CheckedIn {
            return Ok(());
        }

        let Some(mut subscription) = subscription else {
            return Err(RecorderError::SubscribeFailed);
        };

        if let Some(duration) = duration {
            Ok(tokio::time::timeout(duration, subscription.recv()).await??)
        } else {
            Ok(subscription.recv().await?)
        }
    }

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature<
        T: serde::ser::Serialize + serde::de::DeserializeOwned + Send + std::fmt::Debug,
    >(
        &self,
        key: impl Into<String> + std::fmt::Debug,
    ) -> Option<Feature<T>> {
        let key: String = key.into();
        let (tx, rx) = oneshot();

        self.to_configuration_proxy
            .send(ConfigurationProxySignal::GetFeature(key.clone(), tx))
            .instrument(tracing::trace_span!(
                "requesting feature from the configuration proxy"
            ))
            .await
            .inspect_err(|e| tracing::trace!(%e, "Error sending the feature flag request"))
            .ok()?;

        let feature = rx
            .instrument(tracing::trace_span!("waiting for the feature"))
            .await
            .inspect_err(|e| tracing::trace!(%e, "Error requesting the feature flag"))
            .ok()
            .flatten()?;

        self.record(
            "$feature_flag_called",
            Some(Map::from_iter([
                ("$feature_flag".into(), key.into()),
                ("$feature_flag_response".into(), feature.variant.clone()),
            ])),
        )
        .await;

        let variant = feature.variant.clone();
        let payload = if let Some(ref p) = feature.payload {
            let ret = serde_json::from_value(p.clone()).ok()?;
            Some(ret)
        } else {
            None
        };

        Some(Feature { variant, payload })
    }

    pub async fn subscribe_to_feature_changes(
        &self,
    ) -> Option<tokio::sync::broadcast::Receiver<()>> {
        let (tx, rx) = oneshot();

        self.to_configuration_proxy
            .send(ConfigurationProxySignal::Subscribe(tx))
            .instrument(tracing::debug_span!("subscribe to feature changes"))
            .await
            .inspect_err(|e| {
                tracing::error!(error = ?e, "Failed to request subscription to feature changes");
            })
            .ok()?;

        rx.await
            .inspect_err(|e| {
                tracing::error!(error = ?e, "No response when waiting a feature change subscriber");
            })
            .ok()
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub async fn set_fact(
        &self,
        key: impl Into<String> + std::fmt::Debug,
        value: serde_json::Value,
    ) {
        if let Err(e) = self
            .outgoing
            .send(RawSignal::Fact {
                key: key.into(),
                value,
            })
            .await
        {
            tracing::error!(error = ?e, "Failed to enqueue a fact");
        }
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub async fn record(
        &self,
        event: impl Into<String> + std::fmt::Debug,
        properties: Option<Map>,
    ) {
        if let Err(e) = self
            .outgoing
            .send(RawSignal::Event {
                event_name: event.into(),
                properties,
            })
            .instrument(tracing::trace_span!("recording the event"))
            .await
        {
            tracing::error!(error = ?e, "Failed to enqueue an event message");
        }
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub async fn identify(&self, new: DistinctId) {
        self.identify_with_properties(new, IdentifyProperties::default())
            .await;
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub async fn identify_with_properties(&self, new: DistinctId, properties: IdentifyProperties) {
        if let Err(e) = self
            .outgoing
            .send(RawSignal::Identify(new, properties))
            .instrument(tracing::trace_span!("sending the Identify message"))
            .await
        {
            tracing::error!(error = ?e, "Failed to enqueue swap_identity message");
        }

        self.trigger_configuration_refresh()
            .instrument(tracing::trace_span!("triggering a configuration refresh"))
            .await;
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub async fn set_person_properties(&self, properties: IdentifyProperties) {
        if let Err(e) = self
            .outgoing
            .send(RawSignal::SetPersonProperties(properties))
            .instrument(tracing::trace_span!(
                "sending the SetPersonProperties message"
            ))
            .await
        {
            tracing::error!(error = ?e, "Failed to enqueue set_person_properties message");
        }

        self.trigger_configuration_refresh()
            .instrument(tracing::trace_span!("triggering a configuration refresh"))
            .await;
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub async fn add_group(
        &self,
        group_name: impl Into<String> + std::fmt::Debug,
        group_member_id: impl Into<String> + std::fmt::Debug,
    ) {
        if let Err(e) = self
            .outgoing
            .send(RawSignal::AddGroup {
                group_name: group_name.into(),
                group_member_id: group_member_id.into(),
            })
            .instrument(tracing::trace_span!("sending the AddGroup message"))
            .await
        {
            tracing::error!(error = ?e, "Failed to enqueue AddGroup message");
        }

        self.trigger_configuration_refresh()
            .instrument(tracing::trace_span!("triggering a configuration refresh"))
            .await;
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub async fn alias(&self, alias: impl Into<String> + std::fmt::Debug) {
        if let Err(e) = self
            .outgoing
            .send(RawSignal::Alias(alias.into()))
            .instrument(tracing::trace_span!("sending the Alias message"))
            .await
        {
            tracing::error!(error = ?e, "Failed to enqueue Alias message");
        }

        self.trigger_configuration_refresh()
            .instrument(tracing::trace_span!("triggering a configuration refresh"))
            .await;
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub async fn reset(&self) {
        if let Err(e) = self
            .outgoing
            .send(RawSignal::Reset)
            .instrument(tracing::trace_span!("sending the Reset message"))
            .await
        {
            tracing::error!(error = ?e, "Failed to enqueue reset message");
        }

        self.trigger_configuration_refresh()
            .instrument(tracing::trace_span!("triggering a configuration refresh"))
            .await;
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self), ret(level = tracing::Level::TRACE)))]
    async fn get_session_properties(&self) -> Result<Map, FullDuplexError> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.outgoing
            .send(RawSignal::GetSessionProperties { tx })
            .instrument(tracing::trace_span!(
                "sending the GetSessionProperties message"
            ))
            .await
            .map_err(|_| FullDuplexError::SendError)?;

        Ok(rx
            .instrument(tracing::trace_span!("waiting for reply"))
            .await?)
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub async fn flush_now(&self) {
        if let Err(e) = self.outgoing.send(RawSignal::FlushNow).await {
            tracing::error!(error = ?e, "Failed to enqueue a FlushNow message");
        }
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub(crate) async fn trigger_configuration_refresh(&self) {
        if !self.auto_refresh_config {
            tracing::trace!("Not refreshing configuration because it is paused");
            return;
        }

        let (tx, rx) = oneshot();

        let session_properties = self
            .get_session_properties()
            .instrument(tracing::debug_span!("request session properties"))
            .await
            .inspect_err(|e| {
                tracing::debug!(%e, "Failed to get session properties");
            })
            .unwrap_or_default();

        if let Err(e) = self
            .to_configuration_proxy
            .send(ConfigurationProxySignal::CheckInNow(session_properties, tx))
            .instrument(tracing::debug_span!("request immediate check-in"))
            .await
        {
            tracing::error!(error = ?e, "Failed to enqueue CheckInNow message");
        }

        let (config, feats) = match rx
            .instrument(tracing::debug_span!("receive feature facts"))
            .await
        {
            Ok((config, feats)) => (config, feats),
            Err(e) => {
                tracing::error!(error = ?e, "Failed to refresh the configuration");

                return;
            }
        };

        if let Err(e) = self
            .outgoing
            .send(RawSignal::UpdateFeatureConfiguration(config, feats))
            .instrument(tracing::debug_span!("forward feature facts"))
            .await
        {
            tracing::error!(%e, "Failed to forward updated feature facts");
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum FullDuplexError {
    #[error("Failed to request session properties")]
    SendError,

    #[error("Error waiting for a reply: {0}")]
    Recv(#[from] tokio::sync::oneshot::error::RecvError),
}
