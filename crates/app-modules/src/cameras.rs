//! Ustawienia kamer w `core-config` (`camera-hub` — `CameraStore`): przełącznik główny
//! `camera.enabled` (ten sam klucz zmienia strona Ustawień) i nazwy oraz przełączniki urządzeń
//! tej maszyny.

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
