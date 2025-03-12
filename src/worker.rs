use tokio::sync::mpsc::channel;
use tokio::task::JoinHandle;
use tracing::Instrument;

use crate::collator::{Collator, SnapshotError};
use crate::configuration_proxy::{ConfigurationProxy, ConfigurationProxyError};
use crate::ds_correlation::Correlation;
use crate::identity::AnonymousDistinctId;
use crate::storage::Storage;
use crate::submitter::Submitter;
use crate::system_snapshot::SystemSnapshotter;
use crate::transport::Transport;
use crate::{DeviceId, DistinctId, Map, Recorder};

pub struct Worker {
    collator_task: JoinHandle<Result<(), SnapshotError>>,
    submitter_task: JoinHandle<()>,
    configuration_task: JoinHandle<Result<(), ConfigurationProxyError>>,
}

impl Worker {
    #[cfg_attr(
        feature = "tracing-instrument",
        tracing::instrument(skip(
            anonymous_distinct_id,
            distinct_id,
            device_id,
            facts,
            groups,
            system_snapshotter,
            storage,
            transport
        ))
    )]
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn new<F: SystemSnapshotter, P: Storage, T: Transport + Sync + 'static>(
        anonymous_distinct_id: Option<AnonymousDistinctId>,
        distinct_id: Option<DistinctId>,
        device_id: Option<DeviceId>,
        facts: Option<Map>,
        groups: Option<Map>,
        system_snapshotter: F,
        storage: P,
        transport: T,
    ) -> (Recorder, Worker) {
        // Message flow:
        //
        // Recorder --> Configuration --\
        //          `----> Collator -------> Submitter

        let (to_configuration_proxy, configuration_proxy_rx) = channel(1000);
        let (to_collator, collator_rx) = channel(1000);
        let (to_submitter, submitter_rx) = channel(1000);

        let recorder = Recorder::new(to_collator, to_configuration_proxy);
        let configuration = ConfigurationProxy::new(transport.clone(), configuration_proxy_rx);
        let collator = Collator::new(
            system_snapshotter,
            storage,
            collator_rx,
            to_submitter,
            anonymous_distinct_id,
            distinct_id,
            device_id,
            facts.unwrap_or_default(),
            groups.unwrap_or_default(),
            Correlation::import(),
        )
        .await;
        let submitter = Submitter::new(transport, submitter_rx);

        let span = tracing::debug_span!("spawned worker");

        let collator_task = tokio::spawn(collator.execute().instrument(span.clone()));
        let configuration_task = tokio::spawn(configuration.execute().instrument(span.clone()));
        let submitter_task = tokio::spawn(submitter.execute().instrument(span));

        let worker = Self {
            collator_task,
            configuration_task,
            submitter_task,
        };

        recorder
            .trigger_configuration_refresh()
            .instrument(tracing::debug_span!("Initial configuration sync"))
            .await;

        (recorder, worker)
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip(self)))]
    pub async fn wait(self) {
        // Note these three tasks have to shut down in this order.
        //
        // They are also all tokio::spawn'd, so they are all executing in the background, without needing to be awaited.
        //
        // The Submitter won't shut down if the Collator is still running.
        // The ConfigurationProxy and Collator tasks won't shut down if any Recorders are still out there.
        //
        // I'm liking keeping these shut down in this explicit order so we
        // don't accidentally create a more complicated situation where these
        // tasks will (sometimes) never shut down.
        if let Err(e) = self.configuration_task.await {
            tracing::trace!(%e, "IDS Transport configuration task ended with an error");
        }

        if let Err(e) = self.collator_task.await {
            tracing::trace!(%e, "IDS Transport event system_snapshotter ended with an error");
        }

        if let Err(e) = self.submitter_task.await {
            tracing::trace!(%e, "IDS Transport event submitter ended with an error");
        }
    }
}
