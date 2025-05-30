use std::sync::Arc;

use thiserror::Error;
use tokio::sync::mpsc::Receiver;
use tokio::sync::oneshot::Sender as OneshotSender;
use tracing::Instrument;

use crate::{
    checkin::{Checkin, Feature},
    collator::FeatureFacts,
    Map,
};

#[derive(Debug)]
pub(crate) enum ConfigurationProxySignal {
    GetFeature(
        String,
        OneshotSender<Option<Arc<Feature<serde_json::Value>>>>,
    ),
    CheckInNow(Map, OneshotSender<FeatureFacts>),
}

#[derive(Error, Debug)]
pub(crate) enum ConfigurationProxyError {
    #[error("Replying with a collated message failed: {0}")]
    Reply(String),
}

pub(crate) struct ConfigurationProxy<T: crate::transport::Transport> {
    checkin: Option<Checkin>,
    transport: T,
    incoming: Receiver<ConfigurationProxySignal>,
}

impl<T: crate::transport::Transport> ConfigurationProxy<T> {
    pub(crate) fn new(transport: T, incoming: Receiver<ConfigurationProxySignal>) -> Self {
        Self {
            checkin: None,
            transport,
            incoming,
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) async fn execute(mut self) -> Result<(), ConfigurationProxyError> {
        while let Some(signal) = self
            .incoming
            .recv()
            .instrument(tracing::debug_span!("waiting for message"))
            .await
        {
            match signal {
                ConfigurationProxySignal::GetFeature(name, reply) => {
                    self.handle_message_get_feature(name, reply).await?;
                }
                ConfigurationProxySignal::CheckInNow(session_properties, reply) => {
                    self.handle_message_check_in_now(session_properties, reply)
                        .await?;
                }
            }
        }

        Ok(())
    }
    async fn handle_message_get_feature(
        &self,
        name: String,
        reply: OneshotSender<Option<Arc<Feature<serde_json::Value>>>>,
    ) -> Result<(), ConfigurationProxyError> {
        let feat = self
            .checkin
            .as_ref()
            .map(|c| &c.options)
            .as_ref()
            .and_then(|o| o.get(&name))
            .cloned();

        reply
            .send(feat)
            .map_err(|e| ConfigurationProxyError::Reply(format!("{:?}", e)))?;

        Ok(())
    }

    async fn handle_message_check_in_now(
        &mut self,
        session_properties: Map,
        reply: OneshotSender<FeatureFacts>,
    ) -> Result<(), ConfigurationProxyError> {
        if let Ok(fresh_checkin) = self
            .transport
            .checkin(session_properties)
            .await
            .inspect_err(|e| tracing::debug!(%e, "Error refreshing checkin configuration"))
        {
            self.checkin = Some(fresh_checkin);
        }

        let feature_facts = self
            .checkin
            .as_ref()
            .map(|f| f.as_feature_facts())
            .unwrap_or_default();

        reply
            .send(feature_facts)
            .map_err(|e| ConfigurationProxyError::Reply(format!("{:?}", e)))?;

        Ok(())
    }
}
