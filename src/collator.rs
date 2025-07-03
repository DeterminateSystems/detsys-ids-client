use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot::Sender as OneshotSender;
use tracing::Instrument;

use crate::ds_correlation::Correlation;
use crate::identity::{AnonymousDistinctId, DeviceId, DistinctId};
use crate::recorder::RawSignal;
use crate::{Groups, Map};

#[derive(serde::Serialize, Debug)]
pub(crate) enum CollatedSignal {
    Event(Event),
    FlushNow,
}

#[derive(serde::Serialize, Debug)]
pub(crate) struct Event {
    name: String,

    distinct_id: String,
    uuid: uuid::Uuid,
    timestamp: String,

    properties: EventProperties,
}

#[derive(serde::Serialize, Debug)]
struct EventProperties {
    #[serde(rename = "$anon_distinct_id")]
    anon_distinct_id: String,

    #[serde(rename = "$device_id")]
    device_id: String,

    #[serde(rename = "$lib")]
    lib: &'static str,

    #[serde(rename = "$lib_version")]
    lib_version: &'static str,

    #[serde(rename = "$session_id")]
    session_id: String,

    #[serde(rename = "$groups")]
    groups: Groups,

    #[serde(flatten)]
    snapshot: crate::system_snapshot::SystemSnapshot,

    #[serde(flatten)]
    facts: Map,

    #[serde(flatten)]
    featurefacts: FeatureFacts,

    #[serde(flatten)]
    properties: Option<Map>,
}

#[derive(serde::Serialize, Debug, Clone, Default)]
pub(crate) struct FeatureFacts(pub(crate) Map);

#[derive(Error, Debug)]
pub(crate) enum SnapshotError {
    #[error("Forwarding a collated message failed: {0}")]
    Forward(String),

    #[error("Replying with a collated message failed: {0}")]
    Reply(String),
}

pub(crate) struct Collator<F: crate::system_snapshot::SystemSnapshotter, P: crate::storage::Storage>
{
    system_snapshotter: F,
    storage: P,
    incoming: Receiver<RawSignal>,
    outgoing: Sender<CollatedSignal>,
    session_id: String,
    anon_distinct_id: AnonymousDistinctId,
    distinct_id: Option<DistinctId>,
    device_id: DeviceId,
    facts: Map,
    featurefacts: FeatureFacts,
    groups: Groups,
}
impl<F: crate::system_snapshot::SystemSnapshotter, P: crate::storage::Storage> Collator<F, P> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new(
        system_snapshotter: F,
        storage: P,
        incoming: Receiver<RawSignal>,
        outgoing: Sender<CollatedSignal>,
        anonymous_distinct_id: Option<AnonymousDistinctId>,
        distinct_id: Option<DistinctId>,
        device_id: Option<DeviceId>,
        mut facts: Map,
        mut groups: Groups,
        mut correlation_data: Correlation,
    ) -> Self {
        facts.append(&mut correlation_data.properties);
        groups.extend(correlation_data.groups_as_hashmap());

        let stored_ident = storage.load().await.ok().flatten();

        Self {
            system_snapshotter,
            storage,
            incoming,
            outgoing,
            session_id: correlation_data
                .session_id
                .unwrap_or_else(|| uuid::Uuid::now_v7().to_string()),
            anon_distinct_id: anonymous_distinct_id
                .or_else(|| {
                    stored_ident
                        .as_ref()
                        .map(|props| props.anonymous_distinct_id.clone())
                })
                .or_else(|| {
                    correlation_data
                        .anon_distinct_id
                        .map(AnonymousDistinctId::from)
                })
                .unwrap_or_else(|| AnonymousDistinctId::from(uuid::Uuid::now_v7().to_string())),
            distinct_id: distinct_id
                .or_else(|| {
                    stored_ident
                        .as_ref()
                        .and_then(|props| props.distinct_id.clone())
                })
                .or(correlation_data.distinct_id),
            device_id: device_id
                .or_else(|| stored_ident.map(|props| props.device_id))
                .or(correlation_data.device_id)
                .unwrap_or_default(),
            facts,
            featurefacts: FeatureFacts::default(),
            groups,
        }
    }
}

impl<F: crate::system_snapshot::SystemSnapshotter, P: crate::storage::Storage> Collator<F, P> {
    fn distinct_id(&self) -> String {
        if let Some(ref distinct_id) = self.distinct_id {
            distinct_id.to_string()
        } else {
            self.anon_distinct_id.to_string()
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) async fn execute(mut self) -> Result<(), SnapshotError> {
        while let Some(signal) = self
            .incoming
            .recv()
            .instrument(tracing::trace_span!("waiting for RawSignal messages"))
            .await
        {
            match signal {
                RawSignal::GetSessionProperties { tx } => {
                    self.handle_message_get_session_properties(tx).await?;
                }
                RawSignal::Fact { key, value } => {
                    self.handle_message_fact(key, value);
                }
                RawSignal::UpdateFeatureFacts(featurefacts) => {
                    self.handle_message_update_feature_facts(featurefacts);
                }
                RawSignal::Event {
                    event_name,
                    properties,
                } => {
                    self.handle_message_event(event_name, properties).await?;
                }
                RawSignal::Identify(new) => {
                    self.handle_message_identify(new).await?;
                }
                RawSignal::AddGroup {
                    group_name,
                    group_member_id,
                } => {
                    self.handle_message_add_group(group_name, group_member_id)
                        .await?;
                }
                RawSignal::Alias(alias) => {
                    self.handle_message_alias(alias).await?;
                }
                RawSignal::Reset => {
                    self.handle_message_reset().await?;
                }
                RawSignal::FlushNow => {
                    self.handle_message_flush_now().await?;
                }
            }
        }

        self.outgoing
            .send(CollatedSignal::FlushNow)
            .instrument(tracing::trace_span!("final FlushNow"))
            .await
            .map_err(|e| SnapshotError::Forward(format!("{:?}", e)))?;

        Ok(())
    }

    #[cfg_attr(
        feature = "tracing-instrument",
        tracing::instrument(skip_all, fields(event, properties))
    )]
    fn msg_to_event(
        &self,
        snapshot: crate::system_snapshot::SystemSnapshot,
        event: String,
        properties: Option<Map>,
    ) -> Event {
        Event {
            distinct_id: self.distinct_id(),
            name: event,

            properties: EventProperties {
                anon_distinct_id: self.anon_distinct_id.to_string(),
                session_id: self.session_id.to_string(),
                device_id: self.device_id.to_string(),
                snapshot,
                facts: self.facts.clone(),
                featurefacts: self.featurefacts.clone(),
                lib: env!("CARGO_PKG_NAME"),
                lib_version: env!("CARGO_PKG_VERSION"),
                properties,
                groups: self.groups.clone(),
            },

            timestamp: {
                let now: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
                now.to_rfc3339()
            },
            uuid: uuid::Uuid::new_v4(),
        }
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all, ret(level = tracing::Level::TRACE)))]
    async fn handle_message_get_session_properties(
        &self,
        tx: OneshotSender<Map>,
    ) -> Result<(), SnapshotError> {
        let mut props = Map::new();

        if let Ok(person_properties) =
            serde_json::to_value(self.system_snapshotter.snapshot().await)
        {
            props.insert("person_properties".into(), person_properties);
        }

        props.insert("distinct_id".into(), self.distinct_id().into());
        props.insert(
            "$anon_distinct_id".into(),
            self.anon_distinct_id.to_string().into(),
        );
        props.insert(
            "groups".into(),
            serde_json::Value::from_iter(self.groups.clone()),
        );

        tx.send(props)
            .map_err(|e| SnapshotError::Reply(format!("{:?}", e)))?;

        Ok(())
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    fn handle_message_fact(&mut self, key: String, value: serde_json::Value) {
        self.facts.insert(key, value);
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    fn handle_message_update_feature_facts(&mut self, facts: FeatureFacts) {
        self.featurefacts = facts;
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    async fn handle_message_event(
        &self,
        event_name: String,
        properties: Option<Map>,
    ) -> Result<(), SnapshotError> {
        let snapshot = self.system_snapshotter.snapshot().await;
        self.outgoing
            .send(CollatedSignal::Event(
                self.msg_to_event(snapshot, event_name, properties),
            ))
            .await
            .map_err(|e| SnapshotError::Forward(format!("{:?}", e)))?;

        Ok(())
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    async fn handle_message_identify(&mut self, new: DistinctId) -> Result<(), SnapshotError> {
        let old = std::mem::replace(&mut self.distinct_id, Some(new));

        if old.is_some() {
            // Reset our anon distinct ID so we don't link the old id to the new id
            self.anon_distinct_id = AnonymousDistinctId::from(uuid::Uuid::now_v7().to_string());
        }

        if let Err(e) = self
            .storage
            .store(crate::storage::StoredProperties {
                distinct_id: self.distinct_id.clone(),
                anonymous_distinct_id: self.anon_distinct_id.clone(),
                device_id: self.device_id.clone(),
                groups: self.groups.clone(),
            })
            .await
        {
            tracing::debug!(%e, "Storage error");
        }

        let snapshot = self.system_snapshotter.snapshot().await;

        self.outgoing
            .send(CollatedSignal::Event(self.msg_to_event(
                snapshot,
                "$identify".to_string(),
                None,
            )))
            .await
            .map_err(|e| SnapshotError::Forward(format!("{:?}", e)))?;

        Ok(())
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    async fn handle_message_add_group(
        &mut self,
        group_name: String,
        group_member_id: String,
    ) -> Result<(), SnapshotError> {
        self.groups.insert(group_name, group_member_id);

        if let Err(e) = self
            .storage
            .store(crate::storage::StoredProperties {
                distinct_id: self.distinct_id.clone(),
                anonymous_distinct_id: self.anon_distinct_id.clone(),
                device_id: self.device_id.clone(),
                groups: self.groups.clone(),
            })
            .await
        {
            tracing::debug!(%e, "Storage error");
        }

        Ok(())
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    async fn handle_message_alias(&self, alias: String) -> Result<(), SnapshotError> {
        let mut properties = Map::new();

        properties.insert("alias".to_string(), alias.into());

        let snapshot = self.system_snapshotter.snapshot().await;

        self.outgoing
            .send(CollatedSignal::Event(self.msg_to_event(
                snapshot,
                "$create_alias".to_string(),
                Some(properties),
            )))
            .await
            .map_err(|e| SnapshotError::Forward(format!("{:?}", e)))?;

        Ok(())
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    async fn handle_message_reset(&mut self) -> Result<(), SnapshotError> {
        self.distinct_id = None;
        self.anon_distinct_id = AnonymousDistinctId::new();

        if let Err(e) = self
            .storage
            .store(crate::storage::StoredProperties {
                distinct_id: self.distinct_id.clone(),
                anonymous_distinct_id: self.anon_distinct_id.clone(),
                device_id: self.device_id.clone(),
                groups: self.groups.clone(),
            })
            .await
        {
            tracing::debug!(%e, "Storage error");
        }

        Ok(())
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    async fn handle_message_flush_now(&self) -> Result<(), SnapshotError> {
        self.outgoing
            .send(CollatedSignal::FlushNow)
            .await
            .map_err(|e| SnapshotError::Forward(format!("{:?}", e)))?;
        Ok(())
    }
}
