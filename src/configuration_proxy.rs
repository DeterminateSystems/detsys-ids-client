use std::sync::Arc;

use thiserror::Error;
use tokio::sync::RwLock;
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
    QueryIfCheckedIn(OneshotSender<CheckinStatus>),
    GetFeature(
        String,
        OneshotSender<Option<Arc<Feature<serde_json::Value>>>>,
    ),
    CheckInNow(Map, OneshotSender<(Option<Checkin>, FeatureFacts)>),
    Subscribe(OneshotSender<broadcast::Receiver<()>>),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CheckinStatus {
    CheckedIn,
    NotYet,
}

type CheckInPropsWithReply = (Map, OneshotSender<(Option<Checkin>, FeatureFacts)>);

#[derive(Error, Debug)]
pub(crate) enum ConfigurationProxyError {
    #[error("Replying with a collated message failed: {0}")]
    Reply(String),

    #[error(transparent)]
    CollatorSendError(#[from] mpsc::error::SendError<RawSignal>),

    #[error(transparent)]
    CollatorRecvError(#[from] tokio::sync::oneshot::error::RecvError),

    #[error(transparent)]
    BackgroundCheckinSend(#[from] mpsc::error::SendError<CheckInPropsWithReply>),
}

pub(crate) struct ConfigurationProxy<T: crate::transport::Transport> {
    checkin: RwLock<Option<Checkin>>,
    transport: T,
    incoming: Option<mpsc::Receiver<ConfigurationProxySignal>>,
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
            checkin: None.into(),
            transport,
            incoming: Some(incoming),
            collator,
            change_notifier: broadcast::Sender::new(1),
        }
    }

    pub(crate) async fn bootstrap_checkin(&mut self, checkin: Option<Checkin>) {
        let mut c = self.checkin.write().await;
        *c = checkin;
    }

    #[tracing::instrument(skip(self))]
    pub(crate) async fn execute(mut self) -> Result<(), ConfigurationProxyError> {
        let incoming = self.incoming.take().expect("Incoming stream is None");

        let (checkin_trigger, checkin_rx) = mpsc::channel::<CheckInPropsWithReply>(100);

        tokio::select! {
            biased;
            e = self.execute_incoming_worker(incoming, checkin_trigger) => {
                return e;
            },
            e = self.execute_checkin_worker(checkin_rx) => {
                return e;
            }
        };
    }

    #[tracing::instrument(skip(self))]
    pub(crate) async fn execute_incoming_worker(
        &self,
        mut incoming: mpsc::Receiver<ConfigurationProxySignal>,
        checkin_trigger: mpsc::Sender<CheckInPropsWithReply>,
    ) -> Result<(), ConfigurationProxyError> {
        loop {
            let event = incoming.recv().await;
            let Some(event) = event else {
                tracing::debug!("Configuration proxy clients hung up, shutting down");

                return Ok(());
            };

            match event {
                ConfigurationProxySignal::QueryIfCheckedIn(reply) => {
                    self.handle_message_query_if_checked_in(reply).await?;
                }
                ConfigurationProxySignal::GetFeature(name, reply) => {
                    self.handle_message_get_feature(name, reply).await?;
                }
                ConfigurationProxySignal::CheckInNow(session_properties, reply) => {
                    checkin_trigger.send((session_properties, reply)).await?;
                }
                ConfigurationProxySignal::Subscribe(reply) => {
                    self.handle_message_subscribe(reply).await?;
                }
            }
        }
    }

    async fn execute_checkin_worker(
        &self,
        mut checkin_rx: mpsc::Receiver<CheckInPropsWithReply>,
    ) -> Result<(), ConfigurationProxyError> {
        let mut refresh_interval =
            tokio::time::interval(std::time::Duration::from_secs(60 * 60 * 2));
        refresh_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                biased;
                event = checkin_rx.recv() => {
                    let Some((session_properties, reply)) = event else {
                        tracing::debug!("Incoming worker hung up, shutting down");

                        return Ok(());
                    };

                    self.handle_message_check_in_now(session_properties, reply).await?;
                    refresh_interval.reset();
                }
                _ = refresh_interval.tick() => {
                    tracing::debug!("Checking in after the refresh interval ticked");
                    self.check_in_now().await?;
                }
            }
        }
    }

    async fn handle_message_query_if_checked_in(
        &self,
        reply: OneshotSender<CheckinStatus>,
    ) -> Result<(), ConfigurationProxyError> {
        let status = self
            .checkin
            .read()
            .await
            .as_ref()
            .map(|_| CheckinStatus::CheckedIn)
            .unwrap_or(CheckinStatus::NotYet);

        reply
            .send(status)
            .map_err(|e| ConfigurationProxyError::Reply(format!("{e:?}")))?;

        Ok(())
    }

    async fn handle_message_get_feature(
        &self,
        name: String,
        reply: OneshotSender<Option<Arc<Feature<serde_json::Value>>>>,
    ) -> Result<(), ConfigurationProxyError> {
        let feat = self
            .checkin
            .read()
            .await
            .as_ref()
            .map(|c| &c.options)
            .as_ref()
            .and_then(|o| o.get(&name))
            .cloned();

        reply
            .send(feat)
            .map_err(|e| ConfigurationProxyError::Reply(format!("{e:?}")))?;

        Ok(())
    }

    async fn check_in_now(&self) -> Result<(), ConfigurationProxyError> {
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
        &self,
        session_properties: Map,
        reply: OneshotSender<(Option<Checkin>, FeatureFacts)>,
    ) -> Result<(), ConfigurationProxyError> {
        let fresh_checkin: Option<Checkin> = self
            .transport
            .checkin(session_properties)
            .await
            .inspect_err(|e| tracing::debug!(%e, "Error refreshing checkin configuration"))
            .ok();

        let mut current_checkin = self.checkin.write().await;

        let changed = fresh_checkin.is_some() && fresh_checkin != *current_checkin;

        tracing::trace!(
            changed,
            cached = ?current_checkin,
            fresh = ?fresh_checkin,
            "Checked in"
        );

        if changed {
            if let Some(fresh) = fresh_checkin {
                current_checkin.replace(fresh);
            }
        }

        let current_checkin = current_checkin.downgrade().clone();

        let feature_facts = current_checkin
            .as_ref()
            .map(|f| f.as_feature_facts())
            .unwrap_or_default();

        reply
            .send((current_checkin.clone(), feature_facts))
            .map_err(|e| ConfigurationProxyError::Reply(format!("{e:?}")))?;

        if changed {
            if let Err(e) = self.change_notifier.send(()) {
                tracing::debug!(%e, "Error notifying subscribers to changed feature configuration");
            }
        }

        Ok(())
    }

    async fn handle_message_subscribe(
        &self,
        reply: OneshotSender<broadcast::Receiver<()>>,
    ) -> Result<(), ConfigurationProxyError> {
        reply
            .send(self.change_notifier.subscribe())
            .map_err(|e| ConfigurationProxyError::Reply(format!("{e:?}")))?;

        Ok(())
    }
}
