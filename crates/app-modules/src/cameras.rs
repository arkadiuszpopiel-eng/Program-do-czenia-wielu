//! Ustawienia kamer w `core-config` (`camera-hub` — `CameraStore`): przełącznik główny
//! `camera.enabled` (ten sam klucz zmienia strona Ustawień) i nazwy oraz przełączniki urządzeń
//! tej maszyny.

use std::io::Write as _;

use app_api::dto::{CameraKindView, CameraStateView, CameraView, CamerasState};
use app_api::{AppError, ErrorCode};
use base64::Engine;
use camera_hub_contract::{CameraError, CameraInfo, CameraKind, CameraState, HubState, RgbaImage};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use platform_contract::{encode_png_with, zlib_stored};
use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use camera_hub_contract::{CameraStore, StoredCamera, StoredCameras};
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, MachineId, Origin, Scope};
use core_config_impl::FileConfigStore;

/// Przełącznik główny kamer (bool; ten sam klucz przełącza strona Ustawień).
pub const MASTER_KEY: &str = "camera.enabled";
/// Nazwy i przełączniki kamer tej maszyny: tekst JSON `{ id: { name?, enabled } }` (jak
/// `ui.layout` — identyfikatory urządzeń nie są poprawnymi segmentami kluczy konfiguracji).
pub const DEVICES_KEY: &str = "camera.devices";

/// Ustawienia kamer w `core-config`.
pub struct ConfigCameraStore {
    config: Arc<FileConfigStore>,
    machine: MachineId,
    /// Warstwa przełącznika głównego — ta sama, do której pisze `settings_set`.
    master_per_machine: bool,
}

impl ConfigCameraStore {
    async fn get(&self, key: &str) -> Option<serde_json::Value> {
        let key = ConfigKey::new(key).ok()?;
        self.config.get(&key, &Scope::Global).await.ok().flatten()
    }

    async fn set(
        &self,
        key: &str,
        value: Option<serde_json::Value>,
        machine: bool,
    ) -> Result<(), String> {
        let key = ConfigKey::new(key).map_err(|e| e.to_string())?;
        let layer = if machine {
            ConfigLayer::Machine(self.machine.clone())
        } else {
            ConfigLayer::Shared
        };
        self.config
            .set(&key, value, &Scope::Global, &layer, Origin::User)
            .await
            .map_err(|e| e.to_string())
    }
}

#[async_trait]
impl CameraStore for ConfigCameraStore {
    async fn load(&self) -> StoredCameras {
        let master = self.get(MASTER_KEY).await.and_then(|v| v.as_bool());
        let devices = self.get(DEVICES_KEY).await;
        let cameras = devices
            .as_ref()
            .and_then(|v| v.as_str())
            .and_then(|text| serde_json::from_str::<BTreeMap<String, StoredCamera>>(text).ok());
        StoredCameras {
            master: master.unwrap_or(false),
            cameras: cameras.unwrap_or_default(),
        }
    }

    async fn save(&self, data: &StoredCameras) -> Result<(), String> {
        let current = self.load().await;
        if current.master != data.master {
            let value = Some(data.master.into());
            self.set(MASTER_KEY, value, self.master_per_machine).await?;
        }
        if current.cameras != data.cameras {
            let value = if data.cameras.is_empty() {
                None
            } else {
                let text = serde_json::to_string(&data.cameras).map_err(|e| e.to_string())?;
                Some(text.into())
            };
            self.set(DEVICES_KEY, value, true).await?;
        }
        Ok(())
    }
}

impl ConfigCameraStore {
    /// Zapis w konfiguracji; `master_per_machine` — warstwa przełącznika głównego (maszyna).
    pub fn new(config: Arc<FileConfigStore>, machine: MachineId, master_per_machine: bool) -> Self {
        Self {
            config,
            machine,
            master_per_machine,
        }
    }
}

/// Kamera → DTO.
pub fn camera_view(camera: &CameraInfo) -> CameraView {
    let (state, detail, width, height) = match &camera.state {
        CameraState::Asleep => (CameraStateView::Asleep, None, 0, 0),
        CameraState::Opening => (CameraStateView::Opening, None, 0, 0),
        CameraState::Live { width, height } => (CameraStateView::Live, None, *width, *height),
        CameraState::Closing => (CameraStateView::Closing, None, 0, 0),
        CameraState::Failed(text) => (CameraStateView::Failed, Some(text.clone()), 0, 0),
        CameraState::Unplugged => (CameraStateView::Unplugged, None, 0, 0),
    };
    CameraView {
        id: camera.id.0.clone(),
        name: camera.name.clone(),
        kind: match camera.kind {
            CameraKind::Usb => CameraKindView::Usb,
            CameraKind::Network => CameraKindView::Network,
        },
        enabled: camera.enabled,
        state,
        detail,
        width,
        height,
    }
}

/// Stan huba → DTO.
pub fn cameras_dto(state: &HubState) -> CamerasState {
    CamerasState {
        enabled: state.enabled,
        cameras: state.cameras.iter().map(camera_view).collect(),
    }
}

/// Błąd huba → błąd komendy (z powodem dla interfejsu).
pub fn camera_error(error: CameraError) -> AppError {
    match error {
        CameraError::Disabled => AppError::forbidden(
            "Kamery są wyłączone — włącz przełącznik kamer i przełącznik tej kamery.",
        )
        .with("reason", "disabled"),
        CameraError::UnknownCamera(id) => AppError::missing("camera", id, "Nie ma takiej kamery."),
        CameraError::Store(text) => AppError::storage(text),
        CameraError::Unplugged => unavailable("unplugged", "Kamera jest odłączona."),
        CameraError::Stopped => unavailable("stopped", "Kamera została zatrzymana."),
        CameraError::Busy => unavailable("busy", "Kamera właśnie się otwiera — spróbuj za chwilę."),
        CameraError::Timeout => {
            unavailable("timeout", "Kamera nie odpowiedziała w wyznaczonym czasie.")
        }
        CameraError::Platform(text) => {
            unavailable("platform", format!("Kamera nie działa: {text}"))
        }
    }
}

/// Kamera chwilowo niedostępna; `reason` pozwala interfejsowi rozróżnić przypadki bez czytania tekstu.
fn unavailable(reason: &str, message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::Unavailable, message)
        .with("kind", "camera")
        .with("reason", reason)
}

/// Kompresja zlib dla PNG klatki (poziom szybki — podgląd pobiera klatki kilka razy na sekundę).
pub fn zlib_fast(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::with_capacity(data.len() / 3), Compression::fast());
    if encoder.write_all(data).is_err() {
        return zlib_stored(data);
    }
    encoder.finish().unwrap_or_else(|_| zlib_stored(data))
}

/// Klatka jako PNG w base64.
pub fn png_base64(image: &RgbaImage) -> String {
    base64::engine::general_purpose::STANDARD.encode(encode_png_with(image, &zlib_fast))
}

/// Zmiana `camera.enabled` spoza huba (strona Ustawień, reset, import) → przełącznik główny.
pub fn spawn_master_watch(
    hub: &Arc<dyn camera_hub_contract::CameraHub>,
    config: Arc<FileConfigStore>,
) {
    use futures_util::StreamExt;
    let hub = Arc::downgrade(hub);
    tokio::spawn(async move {
        let Ok(key) = ConfigKey::new(MASTER_KEY) else {
            return;
        };
        let mut changes = config.watch(MASTER_KEY);
        while changes.next().await.is_some() {
            let Some(hub) = hub.upgrade() else {
                break;
            };
            let value = config.get(&key, &Scope::Global).await.ok().flatten();
            let enabled = value.and_then(|v| v.as_bool()).unwrap_or(false);
            if let Err(e) = hub.set_master(enabled).await {
                tracing::warn!(error = %e, "przełącznik kamer z ustawień nie został zastosowany");
            }
        }
    });
}
