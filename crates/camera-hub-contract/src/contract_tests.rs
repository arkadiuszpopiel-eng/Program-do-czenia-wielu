//! Współdzielony test kontraktowy (feature `contract-tests`), uruchamiany na `-impl` (z atrapą
//! `CameraPort`) i na `-fake`. Sprawdza niezmienniki huba: przełączniki, równoległe strumienie,
//! natychmiastowe zamykanie, nazwy i obserwację stanu.

use std::future::Future;
use std::time::Duration;

use crate::{CameraError, CameraHub, CameraId, CameraState, HubState, MIN_FRAME_SIDE};

/// Stanowisko testowe: świeży hub z przełącznikiem głównym wyłączonym i co najmniej dwiema
/// podłączonymi, włączonymi kamerami.
pub struct Rig<H> {
    /// Badany hub.
    pub hub: H,
    /// Identyfikatory podłączonych kamer (≥ 2).
    pub cameras: Vec<CameraId>,
    /// Liczba naprawdę otwartych teraz strumieni (z atrapy portu albo atrapy huba).
    pub open_streams: Box<dyn Fn() -> usize + Send + Sync>,
}

fn is_live(state: &HubState, id: &CameraId) -> bool {
    matches!(
        state.camera(id).map(|c| &c.state),
        Some(CameraState::Live { .. })
    )
}

fn is_asleep(state: &HubState, id: &CameraId) -> bool {
    matches!(
        state.camera(id).map(|c| &c.state),
        Some(CameraState::Asleep)
    )
}

async fn on<H: CameraHub>(hub: &H) -> HubState {
    hub.set_master(true)
        .await
        .unwrap_or_else(|e| panic!("włączenie kamer: {e}"))
}

/// Przełącznik główny wyłączony (stan domyślny): żadna kamera się nie otwiera.
pub async fn master_off_never_opens<H: CameraHub>(rig: Rig<H>) {
    let state = rig.hub.refresh().await.unwrap_or_else(|e| panic!("{e}"));
    assert!(!state.enabled, "przełącznik główny domyślnie wyłączony");
    assert!(state.cameras.len() >= 2);
    for id in &rig.cameras {
        assert_eq!(rig.hub.frame(id, 64).await, Err(CameraError::Disabled));
        assert!(is_asleep(&rig.hub.state().await, id));
    }
    assert_eq!((rig.open_streams)(), 0);
}

/// Kilka kamer działa naraz; klatka mieści się w `max_side`, a kolejne klatki się różnią.
pub async fn frames_and_parallel_streams<H: CameraHub>(rig: Rig<H>) {
    let (a, b) = (&rig.cameras[0], &rig.cameras[1]);
    assert!(on(&rig.hub).await.enabled);
    assert_eq!((rig.open_streams)(), 0, "włączenie niczego nie otwiera");
    let first = rig.hub.frame(a, 64).await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(&first.camera, a);
    assert!(first.taken_ms > 0);
    assert!(first.image.width.max(first.image.height) <= 64);
    assert_eq!(
        first.image.pixels.len(),
        first.image.width as usize * first.image.height as usize * 4
    );
    let second = rig.hub.frame(a, 64).await.unwrap_or_else(|e| panic!("{e}"));
    assert_ne!(
        first.image.pixels, second.image.pixels,
        "klatka jest bieżąca"
    );
    let tiny = rig.hub.frame(b, 1).await.unwrap_or_else(|e| panic!("{e}"));
    assert!(tiny.image.width.max(tiny.image.height) <= MIN_FRAME_SIDE);
    let state = rig.hub.state().await;
    assert!(is_live(&state, a) && is_live(&state, b) && state.any_open());
    assert_eq!((rig.open_streams)(), 2, "każda kamera ma własny strumień");
}

/// Wyłączenie jednej kamery zamyka tylko ją; wyłączona nie otwiera się na żądanie.
pub async fn disabling_one_closes_only_it<H: CameraHub>(rig: Rig<H>) {
    let (a, b) = (&rig.cameras[0], &rig.cameras[1]);
    on(&rig.hub).await;
    for id in [a, b] {
        rig.hub
            .frame(id, 64)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
    }
    let state = rig
        .hub
        .set_enabled(a, false)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((rig.open_streams)(), 1);
    assert!(is_asleep(&state, a) && is_live(&state, b));
    assert_eq!(state.camera(a).map(|c| c.enabled), Some(false));
    assert_eq!(rig.hub.frame(a, 64).await, Err(CameraError::Disabled));
    assert!(rig.hub.frame(b, 64).await.is_ok());
    assert_eq!((rig.open_streams)(), 1);
    rig.hub
        .set_enabled(a, true)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(rig.hub.frame(a, 64).await.is_ok());
    assert_eq!((rig.open_streams)(), 2);
}

/// `stop_all` i wyłączenie przełącznika głównego zamykają wszystko od razu.
pub async fn stop_all_and_master_off_close_everything<H: CameraHub>(rig: Rig<H>) {
    let (a, b) = (&rig.cameras[0], &rig.cameras[1]);
    on(&rig.hub).await;
    for id in [a, b] {
        rig.hub
            .frame(id, 64)
            .await
            .unwrap_or_else(|e| panic!("{e}"));
    }
    rig.hub.stop_all().await;
    assert_eq!((rig.open_streams)(), 0);
    let state = rig.hub.state().await;
    assert!(state.enabled, "stop_all nie zmienia przełączników");
    assert!(is_asleep(&state, a) && is_asleep(&state, b) && !state.any_open());
    assert!(
        rig.hub.frame(a, 64).await.is_ok(),
        "kamera budzi się ponownie"
    );
    let state = rig
        .hub
        .set_master(false)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(!state.enabled && !state.any_open());
    assert_eq!((rig.open_streams)(), 0);
    assert_eq!(rig.hub.frame(a, 64).await, Err(CameraError::Disabled));
    assert_eq!((rig.open_streams)(), 0);
}

/// Nazwy: nadanie, oczyszczenie, powrót do nazwy systemowej; nieznana kamera = błąd.
pub async fn rename_and_unknown_camera<H: CameraHub>(rig: Rig<H>) {
    let a = &rig.cameras[0];
    let before = rig.hub.refresh().await.unwrap_or_else(|e| panic!("{e}"));
    let system = before.camera(a).map(|c| c.name.clone()).unwrap_or_default();
    assert!(!system.is_empty());
    let state = rig
        .hub
        .rename(a, "  kuchnia \n")
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(state.camera(a).map(|c| c.name.as_str()), Some("kuchnia"));
    let state = rig
        .hub
        .rename(a, " ")
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(state.camera(a).map(|c| c.name.clone()), Some(system));
    let ghost = CameraId::from("nie-ma-takiej-kamery");
    let unknown =
        |r: Result<HubState, CameraError>| matches!(r, Err(CameraError::UnknownCamera(_)));
    assert!(unknown(rig.hub.rename(&ghost, "x").await));
    assert!(unknown(rig.hub.set_enabled(&ghost, false).await));
    on(&rig.hub).await;
    assert!(matches!(
        rig.hub.frame(&ghost, 64).await,
        Err(CameraError::UnknownCamera(_))
    ));
    assert_eq!((rig.open_streams)(), 0);
}

/// Czeka (do 10 s) na stan spełniający warunek.
async fn seen(
    changes: &mut tokio::sync::watch::Receiver<HubState>,
    done: impl FnMut(&HubState) -> bool,
) -> bool {
    let wait = changes.wait_for(done);
    matches!(
        tokio::time::timeout(Duration::from_secs(10), wait).await,
        Ok(Ok(_))
    )
}

/// Każda zmiana trafia do obserwatorów; ponowne wyliczenie nie gubi kamer.
pub async fn changes_are_published<H: CameraHub>(rig: Rig<H>) {
    let a = &rig.cameras[0];
    rig.hub.refresh().await.unwrap_or_else(|e| panic!("{e}"));
    let mut changes = rig.hub.changes();
    changes.borrow_and_update();
    on(&rig.hub).await;
    assert!(
        seen(&mut changes, |s| s.enabled).await,
        "zmiana przełącznika głównego"
    );
    rig.hub.frame(a, 64).await.unwrap_or_else(|e| panic!("{e}"));
    assert!(seen(&mut changes, |s| is_live(s, a)).await, "kamera działa");
    rig.hub
        .rename(a, "biurko")
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let named = |s: &HubState| s.camera(a).is_some_and(|c| c.name == "biurko");
    assert!(seen(&mut changes, named).await, "zmiana nazwy");
    let refreshed = rig.hub.refresh().await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(refreshed, rig.hub.state().await);
    for id in &rig.cameras {
        assert!(refreshed.camera(id).is_some());
    }
    assert!(
        is_live(&refreshed, a),
        "wyliczenie nie zamyka działającej kamery"
    );
}

/// Cały zestaw; `factory` daje świeże stanowisko.
pub async fn run_all<H, F, Fut>(factory: F)
where
    H: CameraHub,
    F: Fn() -> Fut,
    Fut: Future<Output = Rig<H>>,
{
    master_off_never_opens(factory().await).await;
    frames_and_parallel_streams(factory().await).await;
    disabling_one_closes_only_it(factory().await).await;
    stop_all_and_master_off_close_everything(factory().await).await;
    rename_and_unknown_camera(factory().await).await;
    changes_are_published(factory().await).await;
}
