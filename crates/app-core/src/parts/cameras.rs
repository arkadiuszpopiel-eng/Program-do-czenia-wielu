//! Kamery (`camera-hub`): hub nad portem kamer systemu z ustawieniami w konfiguracji, most stanu
//! huba do zdarzeń interfejsu (`CamerasChanged`) i śledzenie przełącznika głównego
//! `camera.enabled`, który zmienia też strona Ustawień zwykłą komendą `settings_set`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use async_trait::async_trait;
use camera_hub_contract::{CameraHub, CameraStore, StoredCamera, StoredCameras};
use camera_hub_impl::{CameraHubService, HubConfig};
use core_config_contract::{ConfigKey, ConfigLayer, ConfigStore, MachineId, Origin, Scope};
use core_config_impl::FileConfigStore;
use futures_util::StreamExt;

use super::Kernel;
use crate::dto::{AlfaEvent, SettingScope};
use crate::events::EventHub;
use crate::options::AppOptions;

/// Przełącznik główny kamer (bool; ten sam klucz przełącza strona Ustawień).
pub(crate) const MASTER_KEY: &str = "camera.enabled";
/// Nazwy i przełączniki kamer tej maszyny: tekst JSON `{ id: { name?, enabled } }` (jak
/// `ui.layout` — identyfikatory urządzeń nie są poprawnymi segmentami kluczy konfiguracji).
pub(crate) const DEVICES_KEY: &str = "camera.devices";

/// Hub kamer w rdzeniu.
pub(crate) struct Cameras {
    pub hub: Arc<dyn CameraHub>,
    /// Czy urządzenia były już wyliczone na potrzeby interfejsu (pierwsze `cameras_state`).
    pub listed: AtomicBool,
}

/// Ustawienia kamer w `core-config`.
struct ConfigCameraStore {
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

/// Stan huba → zdarzenia `CamerasChanged` (kończy się razem z hubem).
fn spawn_events(hub: &Arc<dyn CameraHub>, events: EventHub) {
    let mut changes = hub.changes();
    tokio::spawn(async move {
        while changes.changed().await.is_ok() {
            let state = crate::commands::cameras::cameras_dto(&changes.borrow_and_update());
            events.emit(AlfaEvent::CamerasChanged { state });
        }
    });
}

/// Zmiana `camera.enabled` spoza huba (strona Ustawień, reset, import) → przełącznik główny.
fn spawn_master_watch(hub: &Arc<dyn CameraHub>, config: Arc<FileConfigStore>) {
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

/// Składa hub: wstrzyknięty z opcji albo `camera-hub-impl` nad portem kamer (z opcji albo
/// Media Foundation) i magazynem w konfiguracji.
pub(crate) async fn build(options: &AppOptions, kernel: &Kernel) -> Cameras {
    let hub: Arc<dyn CameraHub> = match &options.camera_hub {
        Some(hub) => hub.clone(),
        None => {
            let port: Arc<dyn platform_contract::CameraPort> = match &options.cameras {
                Some(port) => port.clone(),
                None => Arc::new(platform_windows_impl::WinCameras),
            };
            let master = kernel.settings.def(MASTER_KEY);
            let store = ConfigCameraStore {
                config: kernel.config.clone(),
                machine: kernel.machine.clone(),
                master_per_machine: master.is_some_and(|d| d.scope == SettingScope::Machine),
            };
            let mut config = HubConfig::default();
            if let Some(idle) = options.camera_idle {
                config.idle_sleep = idle;
            }
            Arc::new(CameraHubService::start(port, Arc::new(store), config).await)
        }
    };
    spawn_events(&hub, kernel.events.clone());
    spawn_master_watch(&hub, kernel.config.clone());
    Cameras {
        hub,
        listed: AtomicBool::new(false),
    }
}
