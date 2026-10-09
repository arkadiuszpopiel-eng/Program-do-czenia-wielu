//! Hub na atrapie `CameraPort`: zestaw kontraktowy i niezmienniki implementacji (usypianie,
//! zatrzymanie wygrywa z żądaniem w toku, izolacja awarii, odłączenie, trwałość ustawień).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use camera_hub_contract::contract_tests::{self, Rig};
use camera_hub_contract::{
    CameraError, CameraHub, CameraId, CameraState, HubState, MemoryCameraStore, StoredCameras,
};
use camera_hub_impl::{CameraHubService, HubConfig, MODULE_TOML};
use core_registry_contract::{Isolation, ModuleManifest};
use platform_fake::FakeCameras;

const A: &str = "fake-cam-0";
const B: &str = "fake-cam-1";

fn config(idle_ms: u64) -> HubConfig {
    HubConfig {
        idle_sleep: Duration::from_millis(idle_ms),
        read_timeout: Duration::from_secs(2),
        capture_side: 640,
    }
}

async fn hub_on(
    cams: &FakeCameras,
    store: &Arc<MemoryCameraStore>,
    idle_ms: u64,
) -> CameraHubService {
    let hub = CameraHubService::start(Arc::new(cams.clone()), store.clone(), config(idle_ms)).await;
    hub.set_master(true).await.unwrap();
    hub
}

fn state_of(state: &HubState, id: &str) -> CameraState {
    state.camera(&id.into()).unwrap().state.clone()
}

async fn until(hub: &CameraHubService, what: &str, done: impl Fn(&HubState) -> bool) {
    let mut changes = hub.changes();
    let seen = tokio::time::timeout(Duration::from_secs(10), changes.wait_for(|s| done(s))).await;
    assert!(matches!(seen, Ok(Ok(_))), "nie doczekano: {what}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contract_suite() {
    contract_tests::run_all(|| async {
        let cams = FakeCameras::new(2);
        let store = Arc::new(MemoryCameraStore::default());
        let hub = CameraHubService::start(Arc::new(cams.clone()), store, config(60_000)).await;
        assert!(
            hub.state().await.cameras.is_empty(),
            "przed refresh lista jest pusta"
        );
        Rig {
            hub,
            cameras: vec![A.into(), B.into()],
            open_streams: Box::new(move || cams.open_total()),
        }
    })
    .await;
}

#[test]
fn manifest_is_valid_and_matches_crate() {
    let manifest = ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(manifest.id.as_str(), "camera-hub");
    assert_eq!(manifest.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest.isolation, Isolation::InProc);
    assert_eq!(manifest.provides[0].to_string(), "camera-hub-contract@1");
    assert_eq!(HubConfig::default().idle_sleep, Duration::from_secs(20));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_stream_falls_asleep_and_wakes_on_demand() {
    let cams = FakeCameras::new(2);
    let store = Arc::new(MemoryCameraStore::default());
    let hub = hub_on(&cams, &store, 150).await;
    let (a, b) = (CameraId::from(A), CameraId::from(B));
    hub.frame(&a, 64).await.unwrap();
    assert_eq!(cams.open_streams(A), 1);
    // Żądania częstsze niż okres bezczynności utrzymują strumień (jedno otwarcie).
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        hub.frame(&a, 64).await.unwrap();
    }
    assert_eq!((cams.open_streams(A), cams.opened_count(A)), (1, 1));
    hub.frame(&b, 64).await.unwrap();
    until(&hub, "uśpienie obu kamer", |s| !s.any_open()).await;
    assert_eq!(cams.open_total(), 0, "uśpiona kamera jest zamknięta");
    assert_eq!(state_of(&hub.state().await, A), CameraState::Asleep);
    hub.frame(&a, 64).await.unwrap();
    assert_eq!((cams.open_streams(A), cams.opened_count(A)), (1, 2));
    until(&hub, "ponowne uśpienie", |s| !s.any_open()).await;
    assert_eq!(cams.open_total(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stop_wins_over_requests_in_flight() {
    let cams = FakeCameras::new(2);
    let store = Arc::new(MemoryCameraStore::default());
    let hub = hub_on(&cams, &store, 60_000).await;
    let a = CameraId::from(A);
    cams.set_delay(Duration::from_millis(250));
    // Żądanie w toku (otwieranie trwa 250 ms) + stop_all: żądanie kończy się `Stopped`.
    let pending = tokio::spawn({
        let (hub, a) = (hub.clone(), a.clone());
        async move { hub.frame(&a, 64).await }
    });
    until(&hub, "otwieranie", |s| {
        state_of(s, A) == CameraState::Opening
    })
    .await;
    hub.stop_all().await;
    assert_eq!(cams.open_total(), 0, "po stop_all nic nie jest otwarte");
    assert_eq!(pending.await.unwrap(), Err(CameraError::Stopped));
    assert_eq!(state_of(&hub.state().await, A), CameraState::Asleep);
    assert_eq!(
        cams.frames_read(A),
        0,
        "zatrzymane żądanie nie przeczytało klatki"
    );
    // To samo dla wyłączenia przełącznika głównego w trakcie odczytu.
    cams.set_delay(Duration::ZERO);
    hub.frame(&a, 64).await.unwrap();
    cams.set_delay(Duration::from_millis(250));
    let pending = tokio::spawn({
        let (hub, a) = (hub.clone(), a.clone());
        async move { hub.frame(&a, 64).await }
    });
    tokio::time::sleep(Duration::from_millis(60)).await;
    let state = hub.set_master(false).await.unwrap();
    assert!(!state.enabled && !state.any_open());
    assert_eq!(cams.open_total(), 0);
    assert_eq!(pending.await.unwrap(), Err(CameraError::Stopped));
    assert_eq!(hub.frame(&a, 64).await, Err(CameraError::Disabled));
    assert_eq!(cams.open_total(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failure_and_unplug_of_one_camera_leave_the_other_alone() {
    let cams = FakeCameras::new(2);
    let store = Arc::new(MemoryCameraStore::default());
    let hub = hub_on(&cams, &store, 60_000).await;
    let (a, b) = (CameraId::from(A), CameraId::from(B));
    hub.frame(&b, 64).await.unwrap();
    cams.fail_open(A, Some("kamera zajęta przez inną aplikację"));
    let failed = hub.frame(&a, 64).await;
    assert!(matches!(&failed, Err(CameraError::Platform(m)) if m.contains("zajęta")));
    let state = hub.state().await;
    assert!(matches!(state_of(&state, A), CameraState::Failed(m) if m.contains("zajęta")));
    assert!(matches!(
        state_of(&state, B),
        CameraState::Live {
            width: 640,
            height: 480
        }
    ));
    assert!(hub.frame(&b, 64).await.is_ok());
    // Kolejne żądanie ponawia otwarcie.
    cams.fail_open(A, None);
    assert!(hub.frame(&a, 64).await.is_ok());
    cams.fail_read(A, Some("usterka sterownika"));
    assert!(matches!(
        hub.frame(&a, 64).await,
        Err(CameraError::Platform(_))
    ));
    assert_eq!(cams.open_streams(A), 0, "błąd odczytu zamyka strumień");
    cams.fail_read(A, None);
    hub.frame(&a, 64).await.unwrap();
    // Odłączenie w trakcie pracy: żądanie → Unplugged; druga kamera działa dalej.
    cams.unplug(A);
    assert_eq!(hub.frame(&a, 64).await, Err(CameraError::Unplugged));
    assert_eq!(state_of(&hub.state().await, A), CameraState::Unplugged);
    assert_eq!(hub.frame(&a, 64).await, Err(CameraError::Unplugged));
    assert_eq!((cams.open_streams(A), cams.open_streams(B)), (0, 1));
    assert!(hub.frame(&b, 64).await.is_ok());
    // Powrót kamery widać po refresh; odłączenie wykryte przez refresh zamyka strumień.
    cams.plug(A);
    assert_eq!(
        state_of(&hub.refresh().await.unwrap(), A),
        CameraState::Asleep
    );
    assert!(hub.frame(&a, 64).await.is_ok());
    cams.unplug(B);
    let state = hub.refresh().await.unwrap();
    assert_eq!(state_of(&state, B), CameraState::Unplugged);
    assert_eq!(cams.open_streams(B), 0);
    cams.add("fake-cam-9", "Nowa kamera", (320, 240));
    let state = hub.refresh().await.unwrap();
    assert_eq!(state.cameras.len(), 3);
    assert!(state.cameras[2].enabled, "nowa kamera domyślnie włączona");
    cams.fail_devices(Some("usługa niedostępna"));
    assert!(matches!(hub.refresh().await, Err(CameraError::Platform(_))));
    assert_eq!(hub.state().await.cameras.len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn settings_survive_restart_and_store_errors_are_reported() {
    let cams = FakeCameras::new(2);
    let store = Arc::new(MemoryCameraStore::default());
    let hub = hub_on(&cams, &store, 60_000).await;
    let (a, b) = (CameraId::from(A), CameraId::from(B));
    hub.rename(&a, "kuchnia").await.unwrap();
    hub.set_enabled(&b, false).await.unwrap();
    let saves = store.saves();
    hub.rename(&a, "kuchnia").await.unwrap();
    hub.set_master(true).await.unwrap();
    assert_eq!(store.saves(), saves, "brak zmiany = brak zapisu");
    drop(hub);
    // Nowy hub na tym samym magazynie: przełącznik główny włączony ⇒ lista od razu.
    let hub = CameraHubService::start(Arc::new(cams.clone()), store.clone(), config(60_000)).await;
    let state = hub.state().await;
    assert!(state.enabled);
    assert_eq!(state.camera(&a).unwrap().name, "kuchnia");
    assert!(!state.camera(&b).unwrap().enabled);
    assert_eq!(hub.frame(&b, 64).await, Err(CameraError::Disabled));
    assert_eq!(cams.open_total(), 0);
    // Powrót do wartości domyślnych czyści wpisy magazynu.
    hub.rename(&a, "").await.unwrap();
    hub.set_enabled(&b, true).await.unwrap();
    assert_eq!(
        store.snapshot(),
        StoredCameras {
            master: true,
            ..StoredCameras::default()
        }
    );
    // Błąd zapisu: wyłączenie i tak zamyka kamery, a wywołujący dostaje błąd.
    hub.frame(&a, 64).await.unwrap();
    store.fail_saves(Some("dysk pełny"));
    assert_eq!(
        hub.set_master(false).await,
        Err(CameraError::Store("dysk pełny".into()))
    );
    assert_eq!(cams.open_total(), 0);
    assert!(!hub.state().await.enabled);
    assert!(matches!(
        hub.rename(&a, "x").await,
        Err(CameraError::Store(_))
    ));
}
