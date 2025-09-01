use std::{sync::Arc, time::Duration};

use crate::checkin::{Checkin, Feature};
use crate::recorder::RecorderError;
use crate::test::slow_transport::SlowTransport;

#[tokio::test]
async fn test() {
    super::init_tracing();

    let transport = SlowTransport::new(Duration::from_millis(500));
    transport
        .set_checkin(Checkin {
            server_options: crate::checkin::ServerOptions::default(),
            options: [(
                String::from("its-true"),
                Arc::new(Feature {
                    variant: true.into(),
                    payload: None,
                }),
            )]
            .into(),
        })
        .await;

    let (recorder, worker) = crate::Builder::new()
        .build_with(
            transport.clone(),
            crate::system_snapshot::Generic::default(),
            crate::storage::Generic::default(),
        )
        .await;

    let worker = tokio::spawn(worker.wait());

    // Trigger configuration refresh but only give it 1ns to complete before timing out.
    // This trips a bug where the configuration proxy shuts down if the sending recorder hangs up.
    tokio::time::timeout(
        Duration::from_nanos(1),
        recorder.trigger_configuration_refresh(),
    )
    .await
    .unwrap_err();

    assert!(
        !matches!(
            recorder
                .wait_for_checkin(Some(Duration::from_millis(1000)))
                .await,
            Err(RecorderError::Subscription(
                tokio::sync::broadcast::error::RecvError::Closed
            ))
        ),
        "Waiting on checkin should not get a Subscription(Closed) error when a config refresh times out."
    );

    assert!(
        recorder
            .get_feature_variant::<bool>("its-true")
            .await
            .unwrap()
    );

    drop(recorder);
    worker.await.unwrap();
}
