//! Deterministyczna atrapa `CameraHub` (docs/modules/camera-hub/SPEC.md, sekcja „Fake”) dla
//! modułów korzystających z klatek (`vision-describe`, `person-recognition`) i testów interfejsu.
//! Bez portu systemowego i bez zegara: klatki to ruchomy wzór testowy, a test steruje
//! odłączeniem, błędem i upływem okresu bezczynności. Reguły przełączników jak w `-impl`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use camera_hub_contract::{
    CameraError, CameraHub, CameraId, CameraInfo, CameraKind, CameraState, Frame, HubState,
    MIN_FRAME_SIDE, RgbaImage, clean_name,
};
use tokio::sync::watch;

/// Rozmiar klatki wirtualnej kamery.
const NATIVE: (u32, u32) = (640, 480);

struct Cam {
    id: CameraId,
    system_name: String,
    custom_name: Option<String>,
    enabled: bool,
    plugged: bool,
    failure: Option<String>,
    state: CameraState,
    frames: u64,
    seed: u8,
}

impl Cam {
    fn info(&self) -> CameraInfo {
        CameraInfo {
            id: self.id.clone(),
            name: self
                .custom_name
                .clone()
                .unwrap_or_else(|| self.system_name.clone()),
            kind: CameraKind::Usb,
            enabled: self.enabled,
            state: self.state.clone(),
        }
    }

    fn sleep(&mut self) {
        if self.state != CameraState::Unplugged {
            self.state = CameraState::Asleep;
        }
    }
}

#[derive(Default)]
struct Hub {
    master: bool,
    cams: Vec<Cam>,
    clock_ms: u64,
}

/// Atrapa huba kamer.
pub struct FakeCameraHub {
    hub: Mutex<Hub>,
    changes: watch::Sender<HubState>,
}

impl FakeCameraHub {
    /// `count` kamer `fake-cam-0…` („Kamera testowa 1…”), przełącznik główny wyłączony.
    pub fn new(count: usize) -> Self {
        let cams = (0..count)
            .map(|i| Cam {
                id: CameraId(format!("fake-cam-{i}")),
                system_name: format!("Kamera testowa {}", i + 1),
                custom_name: None,
                enabled: true,
                plugged: true,
                failure: None,
                state: CameraState::Asleep,
                frames: 0,
                seed: u8::try_from(i * 40 % 256).unwrap_or(0),
            })
            .collect();
        let hub = Hub {
            master: false,
            cams,
            clock_ms: 1_700_000_000_000,
        };
        let (changes, _) = watch::channel(snapshot(&hub));
        Self {
            hub: Mutex::new(hub),
            changes,
        }
    }

    /// Identyfikatory kamer.
    pub fn ids(&self) -> Vec<CameraId> {
        self.lock().cams.iter().map(|c| c.id.clone()).collect()
    }

    fn lock(&self) -> MutexGuard<'_, Hub> {
        self.hub.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Zmienia stan pod blokadą i ogłasza go obserwatorom, jeśli się zmienił.
    fn change<R>(&self, change: impl FnOnce(&mut Hub) -> R) -> (R, HubState) {
        let mut hub = self.lock();
        let result = change(&mut hub);
        let state = snapshot(&hub);
        drop(hub);
        self.changes.send_if_modified(|current| {
            let changed = *current != state;
            if changed {
                current.clone_from(&state);
            }
            changed
        });
        (result, state)
    }

    fn with_cam(&self, id: &CameraId, change: impl FnOnce(&mut Cam)) {
        self.change(|hub| {
            if let Some(cam) = hub.cams.iter_mut().find(|c| &c.id == id) {
                change(cam);
            }
        });
    }

    /// Odłącza kamerę (strumień znika od razu; stan `Unplugged`).
    pub fn unplug(&self, id: &CameraId) {
        self.with_cam(id, |c| {
            c.plugged = false;
            c.state = CameraState::Unplugged;
        });
    }

    /// Podłącza kamerę z powrotem (widoczne po `refresh`).
    pub fn plug(&self, id: &CameraId) {
        self.with_cam(id, |c| c.plugged = true);
    }

    /// Błąd kolejnych żądań klatki tej kamery (`None` — kamera działa).
    pub fn fail(&self, id: &CameraId, error: Option<&str>) {
        self.with_cam(id, |c| c.failure = error.map(str::to_owned));
    }

    /// Upływ okresu bezczynności: wszystkie działające kamery zasypiają.
    pub fn elapse_idle(&self) {
        self.change(|hub| hub.cams.iter_mut().for_each(Cam::sleep));
    }

    /// Liczba otwartych teraz strumieni.
    pub fn open_streams(&self) -> usize {
        let hub = self.lock();
        let live = |c: &&Cam| matches!(c.state, CameraState::Live { .. });
        hub.cams.iter().filter(live).count()
    }
}

fn snapshot(hub: &Hub) -> HubState {
    HubState {
        enabled: hub.master,
        cameras: hub.cams.iter().map(Cam::info).collect(),
    }
}

fn unknown(id: &CameraId) -> CameraError {
    CameraError::UnknownCamera(id.0.clone())
}

/// Wzór testowy pomniejszony do `max_side`; numer klatki przesuwa gradient.
fn pattern(max_side: u32, seed: u8, frame: u64) -> RgbaImage {
    let side = max_side.max(MIN_FRAME_SIDE).min(NATIVE.0);
    let (w, h) = (side, (side * NATIVE.1 / NATIVE.0).max(1));
    let shift = u32::try_from(frame % 256).unwrap_or(0);
    let mut pixels = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let r = u8::try_from((x + shift) % 256).unwrap_or(0);
            let g = u8::try_from((y + shift * 2) % 256).unwrap_or(0);
            pixels.extend_from_slice(&[r, g, seed, 255]);
        }
    }
    RgbaImage {
        width: w,
        height: h,
        pixels,
    }
}

#[async_trait]
impl CameraHub for FakeCameraHub {
    async fn state(&self) -> HubState {
        snapshot(&self.lock())
    }

    async fn set_master(&self, enabled: bool) -> Result<HubState, CameraError> {
        let ((), state) = self.change(|hub| {
            hub.master = enabled;
            if !enabled {
                hub.cams.iter_mut().for_each(Cam::sleep);
            }
        });
        Ok(state)
    }

    async fn set_enabled(&self, id: &CameraId, enabled: bool) -> Result<HubState, CameraError> {
        let (found, state) = self.change(|hub| {
            let cam = hub.cams.iter_mut().find(|c| &c.id == id)?;
            cam.enabled = enabled;
            if !enabled {
                cam.sleep();
            }
            Some(())
        });
        found.map(|()| state).ok_or_else(|| unknown(id))
    }

    async fn rename(&self, id: &CameraId, name: &str) -> Result<HubState, CameraError> {
        let (found, state) = self.change(|hub| {
            let cam = hub.cams.iter_mut().find(|c| &c.id == id)?;
            cam.custom_name = clean_name(name).filter(|n| *n != cam.system_name);
            Some(())
        });
        found.map(|()| state).ok_or_else(|| unknown(id))
    }

    async fn frame(&self, id: &CameraId, max_side: u32) -> Result<Frame, CameraError> {
        let (result, _) = self.change(|hub| {
            if !hub.master {
                return Err(CameraError::Disabled);
            }
            hub.clock_ms += 33;
            let taken_ms = hub.clock_ms;
            let cam = hub.cams.iter_mut().find(|c| &c.id == id);
            let cam = cam.ok_or_else(|| unknown(id))?;
            if !cam.enabled {
                return Err(CameraError::Disabled);
            }
            if !cam.plugged || cam.state == CameraState::Unplugged {
                cam.state = CameraState::Unplugged;
                return Err(CameraError::Unplugged);
            }
            if let Some(text) = &cam.failure {
                cam.state = CameraState::Failed(text.clone());
                return Err(CameraError::Platform(text.clone()));
            }
            cam.frames += 1;
            cam.state = CameraState::Live {
                width: NATIVE.0,
                height: NATIVE.1,
            };
            Ok(Frame {
                camera: id.clone(),
                taken_ms,
                image: pattern(max_side, cam.seed, cam.frames),
            })
        });
        result
    }

    async fn stop_all(&self) {
        self.elapse_idle();
    }

    async fn refresh(&self) -> Result<HubState, CameraError> {
        let ((), state) = self.change(|hub| {
            for cam in &mut hub.cams {
                match (cam.plugged, &cam.state) {
                    (false, _) => cam.state = CameraState::Unplugged,
                    (true, CameraState::Unplugged) => cam.state = CameraState::Asleep,
                    _ => {}
                }
            }
        });
        Ok(state)
    }

    fn changes(&self) -> watch::Receiver<HubState> {
        self.changes.subscribe()
    }
}
