use std::sync::Arc;

use thiserror::Error;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::oneshot::Sender as OneshotSender;
use tracing::Instrument;

use crate::recorder::RawSignal;
use crate::{
    Map,
    checkin::{Checkin, Feature},
    collator::FeatureFacts,
};

#[derive(Debug)]
pub(crate) enum ConfigurationProxySignal {
    GetFeature(
        String,
        OneshotSender<Option<Arc<Feature<serde_json::Value>>>>,
    ),
    CheckInNow(Map, OneshotSender<FeatureFacts>),
    Subscribe(OneshotSender<broadcast::Receiver<()>>),
}

#[derive(Error, Debug)]
pub(crate) enum ConfigurationProxyError {
    #[error("Replying with a collated message failed: {0}")]
    Reply(String),

    #[error(transparent)]
    CollatorSendError(#[from] mpsc::error::SendError<RawSignal>),

    #[error(transparent)]
    CollatorRecvError(#[from] tokio::sync::oneshot::error::RecvError),
}

pub(crate) struct ConfigurationProxy<T: crate::transport::Transport> {
    checkin: Option<Checkin>,
    transport: T,
    incoming: mpsc::Receiver<ConfigurationProxySignal>,
    collator: mpsc::Sender<crate::recorder::RawSignal>,
    change_notifier: broadcast::Sender<()>,
}

impl<T: crate::transport::Transport> ConfigurationProxy<T> {
    pub(crate) fn new(
        transport: T,
        incoming: mpsc::Receiver<ConfigurationProxySignal>,
        collator: mpsc::Sender<crate::recorder::RawSignal>,
    ) -> Self {
        Self {
            checkin: None,
            transport,
            incoming,
            collator,
            change_notifier: broadcast::Sender::new(1),
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) async fn execute(mut self) -> Result<(), ConfigurationProxyError> {
        let mut refresh_interval =
            tokio::time::interval(std::time::Duration::from_secs(60 * 60 * 2));
        refresh_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        refresh_interval.reset();

        loop {
            tokio::select! {
                biased;
                event = self.incoming.recv() => {
                    let Some(event) = event else {
                        tracing::debug!("Configuration proxy clients hung up, shutting down");

                        return Ok(());
                    };

                    match event {
                        ConfigurationProxySignal::GetFeature(name, reply) => {
                            self.handle_message_get_feature(name, reply).await?;
                        }
                        ConfigurationProxySignal::CheckInNow(session_properties, reply) => {
                            self.handle_message_check_in_now(session_properties, reply)
                                .await?;
                            refresh_interval.reset();
                        }
                        ConfigurationProxySignal::Subscribe(reply) => {
                            self.handle_message_subscribe(reply).await?;
                        }
                    }
                },
                _ = refresh_interval.tick() => {
                    self.check_in_now().await?;
                },


            };
        }
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

    async fn check_in_now(&mut self) -> Result<(), ConfigurationProxyError> {
        let session_properties = {
            let (tx, rx) = tokio::sync::oneshot::channel();

            self.collator
                .send(crate::recorder::RawSignal::GetSessionProperties { tx })
                .instrument(tracing::trace_span!(
                    "sending the GetSessionProperties message"
                ))
                .await
                .inspect_err(|e| tracing::debug!(%e, "Failure requesting session properties"))?;

            rx.instrument(tracing::trace_span!("waiting for reply"))
                .await?
        };

        let (sender, receiver) = oneshot::channel();

        self.handle_message_check_in_now(session_properties, sender)
            .await?;

        let reply = receiver.await?;
        tracing::debug!(?reply, "Checked in after timeout");

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

        if let Err(e) = self.change_notifier.send(()) {
            tracing::debug!(%e, "Error notifying subscribers to changed feature configuration");
        }

        Ok(())
    }

    async fn handle_message_subscribe(
        &mut self,
        reply: OneshotSender<broadcast::Receiver<()>>,
    ) -> Result<(), ConfigurationProxyError> {
        reply
            .send(self.change_notifier.subscribe())
            .map_err(|e| ConfigurationProxyError::Reply(format!("{:?}", e)))?;

        Ok(())
    }
}
