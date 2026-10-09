//! Magazyn ustawień kamer wstrzykiwany do huba: przełącznik główny oraz nazwa i przełącznik
//! każdej kamery. Kompozycja aplikacji podaje magazyn nad konfiguracją; testy — magazyn w pamięci.
//! Ustawienia nie zawierają sekretów ani obrazu.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Zapisane ustawienia jednej kamery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCamera {
    /// Nazwa nadana przez użytkownika (`None` = nazwa systemowa).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Przełącznik kamery.
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

impl Default for StoredCamera {
    /// Nowa kamera: włączona (bramką jest przełącznik główny), z nazwą systemową.
    fn default() -> Self {
        Self {
            name: None,
            enabled: true,
        }
    }
}

/// Zapisane ustawienia huba.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StoredCameras {
    /// Przełącznik główny (domyślnie wyłączony).
    #[serde(default)]
    pub master: bool,
    /// Ustawienia kamer według identyfikatora (tylko kamery zmienione przez użytkownika).
    #[serde(default)]
    pub cameras: BTreeMap<String, StoredCamera>,
}

/// Magazyn ustawień kamer.
#[async_trait]
pub trait CameraStore: Send + Sync {
    /// Wczytuje ustawienia (brak albo uszkodzone dane = wartości domyślne).
    async fn load(&self) -> StoredCameras;
    /// Zapisuje ustawienia; błąd jako opis po polsku.
    async fn save(&self, data: &StoredCameras) -> Result<(), String>;
}

/// Magazyn w pamięci (testy, atrapa huba); potrafi symulować błąd zapisu.
#[derive(Debug, Default)]
pub struct MemoryCameraStore {
    data: Mutex<(StoredCameras, usize, Option<String>)>,
}

impl MemoryCameraStore {
    /// Magazyn z danymi początkowymi.
    pub fn with(data: StoredCameras) -> Self {
        Self {
            data: Mutex::new((data, 0, None)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, (StoredCameras, usize, Option<String>)> {
        self.data.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Bieżąca zawartość.
    pub fn snapshot(&self) -> StoredCameras {
        self.lock().0.clone()
    }

    /// Liczba udanych zapisów.
    pub fn saves(&self) -> usize {
        self.lock().1
    }

    /// Błąd zwracany przez kolejne zapisy (`None` — zapis działa).
    pub fn fail_saves(&self, error: Option<&str>) {
        self.lock().2 = error.map(str::to_owned);
    }

    /// Zmiana z zewnątrz (np. przełącznik w Ustawieniach zapisany obok huba).
    pub fn set_external(&self, data: StoredCameras) {
        self.lock().0 = data;
    }
}

#[async_trait]
impl CameraStore for MemoryCameraStore {
    async fn load(&self) -> StoredCameras {
        self.snapshot()
    }

    async fn save(&self, data: &StoredCameras) -> Result<(), String> {
        let mut guard = self.lock();
        if let Some(error) = &guard.2 {
            return Err(error.clone());
        }
        guard.0 = data.clone();
        guard.1 += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_store_round_trips_and_fails_on_demand() {
        let store = MemoryCameraStore::default();
        assert_eq!(store.load().await, StoredCameras::default());
        let mut data = StoredCameras {
            master: true,
            ..StoredCameras::default()
        };
        data.cameras.insert(
            "cam".into(),
            StoredCamera {
                name: Some("kuchnia".into()),
                enabled: false,
            },
        );
        store.save(&data).await.unwrap();
        assert_eq!((store.load().await, store.saves()), (data.clone(), 1));
        store.fail_saves(Some("dysk pełny"));
        assert_eq!(
            store.save(&StoredCameras::default()).await,
            Err("dysk pełny".into())
        );
        assert_eq!(store.snapshot(), data);
        store.set_external(StoredCameras::default());
        assert!(!MemoryCameraStore::with(store.snapshot()).snapshot().master);
    }

    #[test]
    fn stored_shape_is_stable_and_tolerant() {
        let parsed: StoredCameras =
            serde_json::from_str(r#"{"cameras":{"a":{},"b":{"name":"drzwi","enabled":false}}}"#)
                .unwrap();
        assert!(!parsed.master);
        assert_eq!(parsed.cameras["a"], StoredCamera::default());
        assert_eq!(parsed.cameras["b"].name.as_deref(), Some("drzwi"));
        let text = serde_json::to_string(&parsed.cameras["a"]).unwrap();
        assert_eq!(text, r#"{"enabled":true}"#);
    }
}
