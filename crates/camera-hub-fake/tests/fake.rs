//! Test kontraktowy na atrapie + sterowanie zdarzeniami (odłączenie, błąd, bezczynność).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use async_trait::async_trait;
use camera_hub_contract::contract_tests::{self, Rig};
use camera_hub_contract::{CameraError, CameraHub, CameraId, CameraState, Frame, HubState};
use camera_hub_fake::FakeCameraHub;
use tokio::sync::watch;

/// Współdzielona atrapa jako hub (stanowisko potrzebuje też licznika strumieni).
struct Shared(Arc<FakeCameraHub>);

#[async_trait]
impl CameraHub for Shared {
    async fn state(&self) -> HubState {
        self.0.state().await
    }
    async fn set_master(&self, enabled: bool) -> Result<HubState, CameraError> {
        self.0.set_master(enabled).await
    }
    async fn set_enabled(&self, id: &CameraId, enabled: bool) -> Result<HubState, CameraError> {
        self.0.set_enabled(id, enabled).await
    }
    async fn rename(&self, id: &CameraId, name: &str) -> Result<HubState, CameraError> {
        self.0.rename(id, name).await
    }
    async fn frame(&self, id: &CameraId, max_side: u32) -> Result<Frame, CameraError> {
        self.0.frame(id, max_side).await
    }
    async fn stop_all(&self) {
        self.0.stop_all().await;
    }
    async fn refresh(&self) -> Result<HubState, CameraError> {
        self.0.refresh().await
    }
    fn changes(&self) -> watch::Receiver<HubState> {
        self.0.changes()
    }
}

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|| async {
        let fake = Arc::new(FakeCameraHub::new(2));
        let probe = fake.clone();
        Rig {
            cameras: fake.ids(),
            hub: Shared(fake),
            open_streams: Box::new(move || probe.open_streams()),
        }
    })
    .await;
}

#[tokio::test]
async fn scripted_unplug_failure_and_idle() {
    let fake = FakeCameraHub::new(2);
    let ids = fake.ids();
    let (a, b) = (&ids[0], &ids[1]);
    fake.set_master(true).await.unwrap();
    let frame = fake.frame(a, 320).await.unwrap();
    assert_eq!((frame.image.width, frame.image.height), (320, 240));
    assert_ne!(
        frame.image.pixel(0, 0),
        fake.frame(b, 320).await.unwrap().image.pixel(0, 0),
        "kamery mają różne obrazy"
    );
    assert_eq!(fake.frame(a, 5_000).await.unwrap().image.width, 640);
    fake.elapse_idle();
    assert_eq!(fake.open_streams(), 0);

    fake.fail(a, Some("kamera zajęta"));
    assert_eq!(
        fake.frame(a, 64).await,
        Err(CameraError::Platform("kamera zajęta".into()))
    );
    let failed = fake.state().await;
    assert_eq!(
        failed.camera(a).map(|c| c.state.clone()),
        Some(CameraState::Failed("kamera zajęta".into()))
    );
    assert!(
        fake.frame(b, 64).await.is_ok(),
        "awaria jednej nie dotyka drugiej"
    );
    fake.fail(a, None);
    assert!(fake.frame(a, 64).await.is_ok());

    let mut changes = fake.changes();
    changes.borrow_and_update();
    fake.unplug(b);
    assert!(changes.has_changed().unwrap());
    assert_eq!(fake.frame(b, 64).await, Err(CameraError::Unplugged));
    assert_eq!(fake.open_streams(), 1);
    fake.plug(b);
    assert_eq!(fake.frame(b, 64).await, Err(CameraError::Unplugged));
    let state = fake.refresh().await.unwrap();
    assert_eq!(
        state.camera(b).map(|c| c.state.clone()),
        Some(CameraState::Asleep)
    );
    assert!(fake.frame(b, 64).await.is_ok());
    fake.stop_all().await;
    assert!(!fake.state().await.any_open());
}
