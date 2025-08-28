use tokio::sync::mpsc::Receiver;

use crate::collator::{CollatedSignal, Event};

#[derive(Debug, serde::Serialize)]
pub(crate) struct Batch<'a> {
    sent_at: String,
    batch: &'a [Event],
}

pub(crate) struct Submitter<T: crate::transport::Transport> {
    transport: T,
    incoming: Receiver<CollatedSignal>,
    events: Vec<Event>,
}

impl<T: crate::transport::Transport> Submitter<T> {
    pub(crate) fn new(transport: T, incoming: Receiver<CollatedSignal>) -> Self {
        Self {
            transport,
            incoming,
            events: vec![],
        }
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all))]
    pub(crate) async fn execute(mut self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

        loop {
            if self.incoming.is_closed() && self.incoming.is_empty() {
                break;
            }
            tokio::select! {
                biased;
                _ = interval.tick() => {
                    self.try_flush().await;
                }
                incoming_message = self.incoming.recv() => {
                    match incoming_message {
                        Some(CollatedSignal::Event(event)) => {
                            self.events.push(*event);
                        }
                        Some(CollatedSignal::FlushNow) => {
                            self.try_flush().await;
                            interval.reset();
                        }
                        None => {
                            self.try_flush().await;
                            return;
                        }
                    }
                },
            }
        }
    }

    #[cfg_attr(feature = "tracing-instrument", tracing::instrument(skip_all))]
    async fn try_flush(&mut self) {
        if self.events.is_empty() {
            return;
        }

        let batch = Batch {
            sent_at: {
                let now: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
                now.to_rfc3339()
            },
            batch: &self.events,
        };

        tracing::trace!(?batch, "Submitting batch");

        match self.transport.submit(batch).await {
            Ok(_) => {
                tracing::trace!("submitted events");
                self.events.truncate(0);
            }
            Err(e) => {
                tracing::debug!(?e, "submission error");
            }
        }
    }
}
