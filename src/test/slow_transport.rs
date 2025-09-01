use std::{sync::Arc, time::Duration};

use tokio::sync::Mutex;

use crate::{checkin::Checkin, transport::Transport};

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("Simulated error")]
    Simulated,
}

#[derive(Clone)]
pub(crate) struct SlowTransport {
    duration: Duration,
    checkin_val: Arc<Mutex<Option<Checkin>>>,
}

impl SlowTransport {
    pub(crate) fn new(duration: Duration) -> Self {
        Self {
            duration,
            checkin_val: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) async fn set_checkin(&self, checkin: Checkin) {
        (self.checkin_val.lock().await).replace(checkin);
    }
}

impl Transport for SlowTransport {
    type Error = Error;

    async fn checkin(
        &self,
        _session_properties: crate::Map,
    ) -> Result<crate::checkin::Checkin, Self::Error> {
        tokio::time::sleep(self.duration).await;
        (*self.checkin_val.lock().await)
            .clone()
            .ok_or(Error::Simulated)
    }

    async fn submit(&mut self, _batch: crate::submitter::Batch<'_>) -> Result<(), Self::Error> {
        tokio::time::sleep(self.duration).await;
        Err(Error::Simulated)
    }
}
