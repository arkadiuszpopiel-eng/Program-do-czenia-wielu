//! Kamery (`camera-hub`): hub nad portem kamer systemu z ustawieniami w konfiguracji, most stanu
//! huba do zdarzeń interfejsu (`CamerasChanged`) i śledzenie przełącznika głównego
//! `camera.enabled`, który zmienia też strona Ustawień zwykłą komendą `settings_set`.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use camera_hub_contract::CameraHub;
use camera_hub_impl::{CameraHubService, HubConfig};

use super::Kernel;
use crate::dto::{AlfaEvent, SettingScope};
use crate::events::EventHub;
use crate::options::AppOptions;

pub(crate) use app_modules::cameras::MASTER_KEY;
use app_modules::cameras::{ConfigCameraStore, spawn_master_watch};

/// Hub kamer w rdzeniu.
pub(crate) struct Cameras {
    pub hub: Arc<dyn CameraHub>,
    /// Czy urządzenia były już wyliczone na potrzeby interfejsu (pierwsze `cameras_state`).
    pub listed: AtomicBool,
}

/// Stan huba → zdarzenia `CamerasChanged` (kończy się razem z hubem).
fn spawn_events(hub: &Arc<dyn CameraHub>, events: EventHub) {
    let mut changes = hub.changes();
    tokio::spawn(async move {
        while changes.changed().await.is_ok() {
            let state = app_modules::cameras::cameras_dto(&changes.borrow_and_update());
            events.emit(AlfaEvent::CamerasChanged { state });
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
            let store = ConfigCameraStore::new(
                kernel.config.clone(),
                kernel.machine.clone(),
                master.is_some_and(|d| d.scope == SettingScope::Machine),
            );
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
