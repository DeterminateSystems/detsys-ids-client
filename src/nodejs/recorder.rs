use std::sync::{Arc, Mutex, TryLockError};

use neon::prelude::*;
//use serde::Deserialize;

use crate::{Recorder};

use super::Error;

pub(crate) fn neon_hook(cx: &mut ModuleContext) -> neon::result::NeonResult<()> {
    cx.export_function("recorderSetFact", Recorder::js_set_fact)?;

    Ok(())
}

type JsRecorder = JsBox<Arc<Mutex<Recorder>>>;

impl Recorder {
    fn js_set_fact(mut cx: FunctionContext) -> JsResult<JsPromise> {
        let binding = cx.this::<JsRecorder>()?;
        let mut recorder = binding
            .try_lock()
            .map_err(Error::from)
            .or_else(|err| cx.throw_error(err.to_string()))?;

        let key: String = cx.argument::<JsString>(1)?.value(&mut cx);
        let value: String = cx.argument::<JsString>(2)?.value(&mut cx);


        let t = recorder.add_fact(&key, serde_json::Value::String(value));
        let channel = cx.channel();
        let (deferred, promise) = cx.promise();

        super::runtime(&mut cx)?.spawn(async move {
            let r = t.await;

            deferred.settle_with(&channel, move |mut cx| {
                Ok(cx.undefined())
            })
        });

        Ok(promise)
    }
}

/*

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature_variant<T: serde::de::DeserializeOwned + std::fmt::Debug + Send>(
        &self,
        key: impl Into<String> + std::fmt::Debug,
    ) -> Option<T> {
        serde_json::from_value(self.get_feature::<T>(key).await?.variant)
            .inspect_err(|e| tracing::debug!(%e, "Deserializing feature variant failed"))
            .ok()
    }

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature_ptr_variant<
        T: serde::de::DeserializeOwned + Send + std::fmt::Debug,
    >(
        &self,
        key: impl Into<String> + std::fmt::Debug,
    ) -> Option<T> {
        serde_json::from_value(self.get_feature_ptr::<T>(key).await?.variant)
            .inspect_err(|e| tracing::debug!(%e, "Deserializing feature variant failed"))
            .ok()
    }

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature_payload<T: serde::de::DeserializeOwned + Send + std::fmt::Debug>(
        &self,
        key: impl Into<String> + std::fmt::Debug,
    ) -> Option<T> {
        self.get_feature::<T>(key).await?.payload
    }

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature_ptr_payload<
        T: serde::de::DeserializeOwned + Send + std::fmt::Debug,
    >(
        &self,
        key: impl Into<String> + std::fmt::Debug,
    ) -> Option<T> {
        self.get_feature_ptr::<T>(key).await?.payload
    }

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature_ptr<T: serde::de::DeserializeOwned + Send + std::fmt::Debug>(
        &self,
        key: impl Into<String> + std::fmt::Debug,
    ) -> Option<Feature<T>> {
        let ptr = self.get_feature_payload::<String>(key).await?;
        self.get_feature::<T>(ptr).await
    }

    #[tracing::instrument(skip(self), ret(level = tracing::Level::TRACE))]
    pub async fn get_feature<T: serde::de::DeserializeOwned + Send + std::fmt::Debug>(
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

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub async fn record(&self, event: &str, properties: Option<Map>) {
        if let Err(e) = self
            .outgoing
            .send(RawSignal::Event {
                event_name: event.to_string(),
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
        if let Err(e) = self
            .outgoing
            .send(RawSignal::Identify(new))
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
    pub async fn alias(&self, alias: String) {
        if let Err(e) = self
            .outgoing
            .send(RawSignal::Alias(alias))
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

        let feats = match rx
            .instrument(tracing::debug_span!("receive feature facts"))
            .await
        {
            Ok(feats) => feats,
            Err(e) => {
                tracing::error!(error = ?e, "Failed to refresh the configuration");

                return;
            }
        };

        if let Err(e) = self
            .outgoing
            .send(RawSignal::UpdateFeatureFacts(feats))
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

 */
