use std::{sync::Arc, time::Duration};

use crate::{
    checkin::{Checkin, Feature},
    test::slow_transport::SlowTransport,
};

#[tokio::test]
async fn test() {
    super::init_tracing();

    let transport = SlowTransport::new(Duration::from_secs(0));
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

    recorder.wait_for_checkin(None).await.unwrap();

    assert!(
        recorder
            .get_feature_variant::<bool>("its-true")
            .await
            .unwrap()
    );

    drop(recorder);
    worker.await.unwrap();
}
