//! Rejestr kamer w pamięci: wpisy, scalanie z listą urządzeń systemu, migawka dla obserwatorów
//! i ustawienia do zapisania. Czysta logika — bez portu i bez zadań.

use std::sync::Arc;

use camera_hub_contract::{
    CameraError, CameraId, CameraInfo, CameraKind, CameraState, HubState, StoredCamera,
    StoredCameras,
};
use platform_contract::CameraDevice;
use tokio::time::Instant;

use crate::stream::Slot;

/// Kamera w rejestrze.
pub(crate) struct Entry {
    pub id: CameraId,
    pub system_name: String,
    pub custom_name: Option<String>,
    pub enabled: bool,
    pub present: bool,
    pub state: CameraState,
    pub slot: Arc<Slot>,
    /// Żądania klatki w toku.
    pub in_flight: usize,
    /// Chwila ostatniego żądania (dla usypiania).
    pub last_used: Instant,
    /// Czy działa zadanie usypiające tę kamerę.
    pub sleeper: bool,
}

impl Entry {
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

    /// Unieważnia żądania w toku i usypia wpis; zwraca gniazdo z epoką do zamknięcia strumienia.
    pub(crate) fn stop(&mut self) -> (Arc<Slot>, u64) {
        if self.present {
            // Kamera, która mogła być otwarta, jest „zamykana” do potwierdzenia zamknięcia
            // strumienia ([`Registry::settle`]) — dopiero wtedy stan mówi „uśpiona”.
            let maybe_open = self.in_flight > 0
                || matches!(
                    self.state,
                    CameraState::Live { .. } | CameraState::Opening | CameraState::Closing
                );
            self.state = if maybe_open {
                CameraState::Closing
            } else {
                CameraState::Asleep
            };
        }
        (self.slot.clone(), self.slot.invalidate())
    }

    pub(crate) fn is_live(&self) -> bool {
        matches!(self.state, CameraState::Live { .. })
    }
}

/// Stan huba.
#[derive(Default)]
pub(crate) struct Registry {
    pub master: bool,
    /// Ustawienia zapisane (także kamer niewidzianych w tej sesji).
    pub stored: StoredCameras,
    pub cameras: Vec<Entry>,
}

impl Registry {
    pub(crate) fn new(stored: StoredCameras) -> Self {
        Self {
            master: stored.master,
            stored,
            cameras: Vec::new(),
        }
    }

    /// Potwierdza zamknięcie strumieni: wpisy „zamykane”, których gniazdo zostało zamknięte dla
    /// bieżącej epoki (nie było późniejszego zatrzymania), stają się uśpione.
    pub(crate) fn settle(&mut self, closed: &[(Arc<Slot>, u64)]) {
        for entry in &mut self.cameras {
            let confirmed = closed.iter().any(|(slot, epoch)| {
                Arc::ptr_eq(slot, &entry.slot) && entry.slot.epoch() == *epoch
            });
            if confirmed && entry.state == CameraState::Closing {
                entry.state = CameraState::Asleep;
            }
        }
    }

    pub(crate) fn snapshot(&self) -> HubState {
        HubState {
            enabled: self.master,
            cameras: self.cameras.iter().map(Entry::info).collect(),
        }
    }

    pub(crate) fn entry(&mut self, id: &CameraId) -> Result<&mut Entry, CameraError> {
        self.cameras
            .iter_mut()
            .find(|c| &c.id == id)
            .ok_or_else(|| CameraError::UnknownCamera(id.0.clone()))
    }

    /// Zapisuje w ustawieniach przełącznik i nazwę wpisu (wartości domyślne nie zajmują miejsca).
    pub(crate) fn remember(&mut self, id: &CameraId) {
        let Some(entry) = self.cameras.iter().find(|c| &c.id == id) else {
            return;
        };
        let stored = StoredCamera {
            name: entry.custom_name.clone(),
            enabled: entry.enabled,
        };
        if stored == StoredCamera::default() {
            self.stored.cameras.remove(&id.0);
        } else {
            self.stored.cameras.insert(id.0.clone(), stored);
        }
    }

    /// Scala listę urządzeń systemu: nowe kamery dochodzą z zapisanymi ustawieniami (domyślnie
    /// włączone, nazwa systemowa), brakujące stają się odłączone, powracające — uśpione.
    /// Zwraca gniazda kamer odłączonych do zamknięcia.
    pub(crate) fn merge(&mut self, devices: Vec<CameraDevice>) -> Vec<(Arc<Slot>, u64)> {
        let mut closing = Vec::new();
        for entry in &mut self.cameras {
            let found = devices.iter().find(|d| d.id == entry.id.0);
            match (found, entry.present) {
                (Some(device), was_present) => {
                    entry.system_name.clone_from(&device.name);
                    if !was_present {
                        entry.present = true;
                        entry.state = CameraState::Asleep;
                    }
                }
                (None, true) => {
                    entry.present = false;
                    closing.push(entry.stop());
                    entry.state = CameraState::Unplugged;
                }
                (None, false) => {}
            }
        }
        for device in devices {
            if self.cameras.iter().any(|c| c.id.0 == device.id) {
                continue;
            }
            let stored = self.stored.cameras.get(&device.id).cloned();
            let stored = stored.unwrap_or_default();
            self.cameras.push(Entry {
                id: CameraId(device.id),
                system_name: device.name,
                custom_name: stored.name,
                enabled: stored.enabled,
                present: true,
                state: CameraState::Asleep,
                slot: Arc::new(Slot::default()),
                in_flight: 0,
                last_used: Instant::now(),
                sleeper: false,
            });
        }
        closing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, name: &str) -> CameraDevice {
        CameraDevice {
            id: id.to_owned(),
            name: name.to_owned(),
        }
    }

    #[tokio::test]
    async fn merge_applies_stored_settings_and_tracks_hot_plug() {
        let mut stored = StoredCameras::default();
        stored.cameras.insert(
            "b".into(),
            StoredCamera {
                name: Some("drzwi".into()),
                enabled: false,
            },
        );
        let mut reg = Registry::new(stored);
        assert!(
            reg.merge(vec![device("a", "Kamera A"), device("b", "Kamera B")])
                .is_empty()
        );
        let state = reg.snapshot();
        assert_eq!(state.cameras[0].name, "Kamera A");
        assert!(state.cameras[0].enabled && !state.cameras[1].enabled);
        assert_eq!(state.cameras[1].name, "drzwi");
        // Odłączenie A: stan Unplugged i gniazdo do zamknięcia; powrót: Asleep, nowa nazwa.
        let epoch = reg.cameras[0].slot.epoch();
        let closing = reg.merge(vec![device("b", "Kamera B")]);
        assert_eq!(closing.len(), 1);
        assert_eq!(closing[0].1, epoch + 1);
        assert_eq!(reg.snapshot().cameras[0].state, CameraState::Unplugged);
        assert!(reg.merge(vec![device("b", "Kamera B")]).is_empty());
        reg.merge(vec![device("a", "Kamera A2"), device("b", "Kamera B")]);
        let state = reg.snapshot();
        assert_eq!(state.cameras.len(), 2);
        assert_eq!(state.cameras[0].name, "Kamera A2");
        assert_eq!(state.cameras[0].state, CameraState::Asleep);
    }

    #[tokio::test]
    async fn remember_keeps_only_non_default_settings() {
        let mut reg = Registry::new(StoredCameras::default());
        reg.merge(vec![device("a", "Kamera A")]);
        let id = CameraId::from("a");
        reg.remember(&id);
        assert!(reg.stored.cameras.is_empty());
        reg.entry(&id).unwrap().custom_name = Some("biurko".into());
        reg.remember(&id);
        assert_eq!(reg.stored.cameras["a"].name.as_deref(), Some("biurko"));
        reg.entry(&id).unwrap().custom_name = None;
        reg.remember(&id);
        assert!(reg.stored.cameras.is_empty());
        assert!(reg.entry(&CameraId::from("x")).is_err());
        reg.remember(&CameraId::from("x"));
        let live = CameraState::Live {
            width: 2,
            height: 2,
        };
        reg.entry(&id).unwrap().state = live;
        assert!(reg.entry(&id).unwrap().is_live());
        // Zatrzymana kamera, która działała, jest „zamykana” do potwierdzenia zamknięcia strumienia;
        // potwierdzenie dla starszej epoki (było późniejsze zatrzymanie) niczego nie zmienia.
        let (slot, stale) = reg.entry(&id).unwrap().stop();
        assert_eq!(reg.snapshot().cameras[0].state, CameraState::Closing);
        let (_, current) = reg.entry(&id).unwrap().stop();
        reg.settle(&[(slot.clone(), stale)]);
        assert_eq!(reg.snapshot().cameras[0].state, CameraState::Closing);
        reg.settle(&[(slot, current)]);
        assert_eq!(reg.snapshot().cameras[0].state, CameraState::Asleep);
        // Uśpiona kamera po zatrzymaniu zostaje uśpiona (nie było czego zamykać).
        reg.entry(&id).unwrap().stop();
        assert_eq!(reg.snapshot().cameras[0].state, CameraState::Asleep);
    }
}
