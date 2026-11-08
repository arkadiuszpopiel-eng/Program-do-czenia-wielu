//! Komendy `cameras_*` na atrapie portu kamer: przełącznik główny i per kamera, klatka PNG,
//! kilka kamer naraz, zdarzenie `CamerasChanged`, trwałość ustawień, usypianie, kill-switch.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::Read;
use std::sync::Arc;
use std::time::Duration;

use app_core::dto::{AlfaEvent, CameraFrame, CameraStateView, CamerasState, SettingValue};
use app_core::ports::{HeadlessShell, KillOrigin};
use app_core::{AppCore, AppPaths, ErrorCode, EventBatch, MemorySecretStore};
use base64::Engine;
use common::{options, until};
use platform_fake::FakeCameras;
use tokio::sync::broadcast::Receiver;

const A: &str = "fake-cam-0";
const B: &str = "fake-cam-1";

struct Rig {
    core: AppCore,
    cams: FakeCameras,
    rx: Receiver<EventBatch>,
    dir: tempfile::TempDir,
    /// Sejf wspólny dla restartów (klucz baz sesji).
    secrets: Arc<MemorySecretStore>,
}

async fn build(
    dir: &std::path::Path,
    cams: &FakeCameras,
    idle: Duration,
    secrets: &Arc<MemorySecretStore>,
) -> AppCore {
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.cameras = Some(Arc::new(cams.clone()));
    opts.camera_idle = Some(idle);
    opts.secrets = Some(secrets.clone());
    AppCore::build(AppPaths::under(dir), opts).await.unwrap()
}

async fn rig(idle: Duration) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let cams = FakeCameras::new(2);
    let secrets = Arc::new(MemorySecretStore::default());
    let core = build(dir.path(), &cams, idle, &secrets).await;
    let rx = core.subscribe_events();
    Rig {
        core,
        cams,
        rx,
        dir,
        secrets,
    }
}

fn state_of(state: &CamerasState, id: &str) -> CameraStateView {
    state.cameras.iter().find(|c| c.id == id).unwrap().state
}

/// Czeka na `CamerasChanged` spełniające warunek.
async fn cameras_event(
    rx: &mut Receiver<EventBatch>,
    done: impl Fn(&CamerasState) -> bool,
) -> CamerasState {
    let seen = until(rx, |e| match e {
        AlfaEvent::CamerasChanged { state } => done(state),
        _ => false,
    })
    .await;
    seen.into_iter()
        .rev()
        .find_map(|e| match e {
            AlfaEvent::CamerasChanged { state } if done(&state) => Some(state),
            _ => None,
        })
        .unwrap()
}

/// Dekoduje PNG klatki: sprawdza sygnaturę, wymiary z IHDR i rozpakowuje dane obrazu.
fn decode_png(frame: &CameraFrame) -> Vec<u8> {
    let png = base64::engine::general_purpose::STANDARD
        .decode(&frame.png_base64)
        .unwrap();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "sygnatura PNG");
    let (mut at, mut idat, mut size) = (8, Vec::new(), (0u32, 0u32));
    while at + 8 <= png.len() {
        let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
        let (kind, data) = (&png[at + 4..at + 8], &png[at + 8..at + 8 + len]);
        match kind {
            b"IHDR" => {
                size = (
                    u32::from_be_bytes(data[..4].try_into().unwrap()),
                    u32::from_be_bytes(data[4..8].try_into().unwrap()),
                );
                assert_eq!(&data[8..10], [8, 6], "8 bitów, RGBA");
            }
            b"IDAT" => idat.extend_from_slice(data),
            _ => {}
        }
        at += len + 12;
    }
    assert_eq!(size, (frame.width, frame.height), "wymiary z IHDR");
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(idat.as_slice())
        .read_to_end(&mut raw)
        .unwrap();
    let row = 1 + frame.width as usize * 4;
    assert_eq!(raw.len(), row * frame.height as usize, "pełny obraz");
    raw
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn master_off_refuses_frames_and_opens_nothing() {
    let r = rig(Duration::from_secs(60)).await;
    let state = r.core.cameras_state().await.unwrap();
    assert!(!state.enabled, "kamery domyślnie wyłączone");
    assert_eq!(state.cameras.len(), 2);
    assert_eq!(state.cameras[0].name, "Kamera testowa 1");
    assert!(state.cameras.iter().all(|c| c.enabled && c.width == 0));
    assert_eq!(state_of(&state, A), CameraStateView::Asleep);
    let refused = r.core.cameras_frame(A.into(), 320).await.unwrap_err();
    assert_eq!(refused.code, ErrorCode::Forbidden);
    let missing = r
        .core
        .cameras_frame("nie-ma".into(), 320)
        .await
        .unwrap_err();
    assert_eq!(
        missing.code,
        ErrorCode::Forbidden,
        "wyłączone = odmowa przed wszystkim"
    );
    r.core.system_kill_all(KillOrigin::Ui).await;
    assert_eq!((r.cams.open_total(), r.cams.opened_count(A)), (0, 0));
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json["cameras"][0]["state"], "asleep");
    assert_eq!(json["cameras"][0]["kind"], "usb");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn frames_two_live_cameras_disable_and_events() {
    let mut r = rig(Duration::from_secs(60)).await;
    r.core.cameras_state().await.unwrap();
    let state = r.core.cameras_set_master(true).await.unwrap();
    assert!(state.enabled);
    assert_eq!(r.cams.open_total(), 0, "włączenie niczego nie otwiera");

    let frame = r.core.cameras_frame(A.into(), 320).await.unwrap();
    assert_eq!(
        (frame.camera.as_str(), frame.width, frame.height),
        (A, 320, 240)
    );
    assert!(frame.taken_ms > 1_600_000_000_000);
    let first = decode_png(&frame);
    let second = decode_png(&r.core.cameras_frame(A.into(), 320).await.unwrap());
    assert_ne!(first, second, "kolejna klatka jest nowa");
    // `max_side` przycinane do 64–1280 (kamera atrapy ma 640×480).
    let tiny = r.core.cameras_frame(B.into(), 1).await.unwrap();
    assert_eq!((tiny.width, tiny.height), (64, 48));
    let big = r.core.cameras_frame(B.into(), 100_000).await.unwrap();
    assert_eq!((big.width, big.height), (640, 480));
    decode_png(&big);
    assert!(
        big.png_base64.len() < 640 * 480 * 4,
        "PNG jest skompresowany"
    );

    assert_eq!((r.cams.open_streams(A), r.cams.open_streams(B)), (1, 1));
    let both = |s: &CamerasState| {
        state_of(s, A) == CameraStateView::Live && state_of(s, B) == CameraStateView::Live
    };
    let live = cameras_event(&mut r.rx, both).await;
    let a = live.cameras.iter().find(|c| c.id == A).unwrap();
    assert_eq!((a.width, a.height, a.detail.clone()), (640, 480, None));

    // Wyłączenie jednej kamery zamyka tylko ją.
    let state = r.core.cameras_set_enabled(B.into(), false).await.unwrap();
    assert_eq!(state_of(&state, B), CameraStateView::Asleep);
    assert_eq!((r.cams.open_streams(A), r.cams.open_streams(B)), (1, 0));
    let refused = r.core.cameras_frame(B.into(), 320).await.unwrap_err();
    assert_eq!(refused.code, ErrorCode::Forbidden);
    assert!(r.core.cameras_frame(A.into(), 320).await.is_ok());

    // Awaria jednej kamery: stan `failed` z opisem, komenda zwraca błąd.
    r.cams.fail_read(A, Some("usterka sterownika"));
    let failed = r.core.cameras_frame(A.into(), 320).await.unwrap_err();
    assert_eq!(failed.code, ErrorCode::Unavailable);
    assert!(failed.message.contains("usterka sterownika"));
    let state = r.core.cameras_state().await.unwrap();
    let a = state.cameras.iter().find(|c| c.id == A).unwrap();
    assert_eq!(a.state, CameraStateView::Failed);
    assert!(a.detail.as_deref().unwrap().contains("usterka"));
    r.cams.fail_read(A, None);
    assert!(r.core.cameras_frame(A.into(), 320).await.is_ok());

    // Odłączenie i ponowne wyliczenie.
    r.cams.unplug(A);
    let state = r.core.cameras_refresh().await.unwrap();
    assert_eq!(state_of(&state, A), CameraStateView::Unplugged);
    assert_eq!(r.cams.open_total(), 0);
    let gone = r.core.cameras_frame(A.into(), 320).await.unwrap_err();
    assert_eq!(gone.code, ErrorCode::Unavailable);
    let unknown = r.core.cameras_rename("nie-ma".into(), "x".into()).await;
    assert_eq!(unknown.unwrap_err().code, ErrorCode::NotFound);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn settings_toggle_and_kill_switch_close_cameras() {
    let mut r = rig(Duration::from_secs(60)).await;
    r.core.cameras_state().await.unwrap();
    // Przełącznik z Ustawień (zwykłe `settings_set`) włącza hub przez obserwację konfiguracji.
    let key = "camera.enabled".to_owned();
    r.core
        .settings_set(key.clone(), SettingValue::Bool(true))
        .await
        .unwrap();
    cameras_event(&mut r.rx, |s| s.enabled).await;
    r.core.cameras_frame(A.into(), 128).await.unwrap();
    r.core.cameras_frame(B.into(), 128).await.unwrap();
    assert_eq!(r.cams.open_total(), 2);

    // Kill-switch zamyka strumienie, ale nie zmienia przełączników.
    r.core.system_kill_all(KillOrigin::Hotkey).await;
    assert_eq!(r.cams.open_total(), 0);
    let state = r.core.cameras_state().await.unwrap();
    assert!(state.enabled);
    assert_eq!(state_of(&state, A), CameraStateView::Asleep);

    // Wyłączenie w Ustawieniach zamyka wszystko; komenda huba zapisuje ten sam klucz.
    r.core.cameras_frame(A.into(), 128).await.unwrap();
    r.core
        .settings_set(key.clone(), SettingValue::Bool(false))
        .await
        .unwrap();
    cameras_event(&mut r.rx, |s| !s.enabled).await;
    assert_eq!(r.cams.open_total(), 0);
    let refused = r.core.cameras_frame(A.into(), 128).await.unwrap_err();
    assert_eq!(refused.code, ErrorCode::Forbidden);
    r.core.cameras_set_master(true).await.unwrap();
    assert_eq!(
        r.core.settings_values().await.unwrap().get(&key),
        Some(&SettingValue::Bool(true))
    );
    assert_eq!(
        r.core.settings_reset(key).await.unwrap(),
        SettingValue::Bool(false)
    );
    cameras_event(&mut r.rx, |s| !s.enabled).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn names_and_switches_survive_restart() {
    let r = rig(Duration::from_secs(60)).await;
    r.core.cameras_state().await.unwrap();
    r.core.cameras_set_master(true).await.unwrap();
    let state = r
        .core
        .cameras_rename(A.into(), "  kuchnia ".into())
        .await
        .unwrap();
    assert_eq!(state.cameras[0].name, "kuchnia");
    r.core.cameras_set_enabled(B.into(), false).await.unwrap();
    r.core.cameras_frame(A.into(), 128).await.unwrap();
    let Rig {
        core,
        cams,
        rx,
        dir,
        secrets,
    } = r;
    drop(core);
    drop(rx);

    let core = build(dir.path(), &cams, Duration::from_secs(60), &secrets).await;
    let state = core.cameras_state().await.unwrap();
    assert!(state.enabled, "przełącznik główny zapamiętany");
    let a = state.cameras.iter().find(|c| c.id == A).unwrap();
    let b = state.cameras.iter().find(|c| c.id == B).unwrap();
    assert_eq!(
        (a.name.as_str(), a.enabled, b.enabled),
        ("kuchnia", true, false)
    );
    assert_eq!(b.name, "Kamera testowa 2");
    assert_eq!(
        state_of(&state, A),
        CameraStateView::Asleep,
        "po starcie kamery śpią"
    );
    let refused = core.cameras_frame(B.into(), 128).await.unwrap_err();
    assert_eq!(refused.code, ErrorCode::Forbidden);
    // Pusta nazwa przywraca nazwę systemową.
    let state = core.cameras_rename(A.into(), String::new()).await.unwrap();
    assert_eq!(state.cameras[0].name, "Kamera testowa 1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_camera_falls_asleep_and_closes_the_stream() {
    let mut r = rig(Duration::from_millis(200)).await;
    r.core.cameras_state().await.unwrap();
    r.core.cameras_set_master(true).await.unwrap();
    r.core.cameras_frame(A.into(), 128).await.unwrap();
    assert_eq!(r.cams.open_streams(A), 1);
    cameras_event(&mut r.rx, |s| state_of(s, A) == CameraStateView::Live).await;
    let asleep = cameras_event(&mut r.rx, |s| state_of(s, A) == CameraStateView::Asleep).await;
    assert!(asleep.enabled);
    assert_eq!(r.cams.open_total(), 0, "uśpiona kamera jest zamknięta");
    r.core.cameras_frame(A.into(), 128).await.unwrap();
    assert_eq!(r.cams.opened_count(A), 2, "kolejna klatka budzi kamerę");
    drop(r.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn camera_frame_becomes_a_composer_attachment() {
    use app_core::dto::{AttachmentKind, SessionTemplate};
    let r = rig(Duration::from_secs(60)).await;
    let session = r
        .core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    r.core.cameras_state().await.unwrap();
    // Kamery wyłączone: czytelny błąd, nic nie zostaje dodane, kamera się nie otwiera.
    let refused = r
        .core
        .attachments_add_camera(session.clone(), A.into())
        .await
        .unwrap_err();
    assert_eq!(refused.code, ErrorCode::Forbidden);
    assert!(refused.message.contains("Kamery są wyłączone"));
    assert!(
        r.core
            .attachments_list(session.clone())
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(r.cams.opened_count(A), 0);

    r.core.cameras_set_master(true).await.unwrap();
    let added = r
        .core
        .attachments_add_camera(session.clone(), A.into())
        .await
        .unwrap();
    assert!(added.rejected.is_empty(), "{:?}", added.rejected);
    assert_eq!((added.added.len(), added.staged.len()), (1, 1));
    assert_eq!(added.added[0].kind, AttachmentKind::Image);
    assert!(added.added[0].name.ends_with(".png"));
    assert_eq!(
        r.core
            .attachments_list(session.clone())
            .await
            .unwrap()
            .len(),
        1
    );

    r.cams.unplug(A);
    let gone = r
        .core
        .attachments_add_camera(session.clone(), A.into())
        .await
        .unwrap_err();
    assert_eq!(gone.code, ErrorCode::Unavailable);
    let missing = r
        .core
        .attachments_add_camera("nie-ma-sesji".into(), B.into())
        .await;
    assert!(missing.is_err());
    assert_eq!(r.core.attachments_list(session).await.unwrap().len(), 1);
    assert_eq!(r.cams.opened_count(B), 0, "zła sesja nie budzi kamery");
}
