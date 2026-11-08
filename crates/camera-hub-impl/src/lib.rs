//! Implementacja `camera-hub` (docs/modules/camera-hub/SPEC.md, v0: kamery USB przez
//! `CameraPort` z `platform-contract`; moduł nie ma własnego kodu systemowego).
//!
//! Każda kamera ma własne gniazdo strumienia (`stream.rs`); blokujące wywołania portu idą przez
//! `spawn_blocking`, więc kilka kamer działa równolegle, a awaria jednej nie dotyka pozostałych.
//! Strumień otwiera pierwsze żądanie klatki, a zamyka zadanie usypiające po `idle_sleep` bez
//! żądań, wyłączenie kamery albo przełącznika głównego i `stop_all`. Klatki nie są zapisywane.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod state;
mod stream;

use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use camera_hub_contract::{
    CameraError, CameraHub, CameraId, CameraState, CameraStore, Frame, HubState, MIN_FRAME_SIDE,
    RgbaImage, clean_name,
};
use platform_contract::CameraPort;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::state::Registry;
use crate::stream::{ReadFailure, ReadPlan, Slot, read_frame};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Ustawienia huba.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HubConfig {
    /// Po jakim czasie bez żądań klatki strumień się zamyka.
    pub idle_sleep: Duration,
    /// Limit czekania na klatkę z kamery.
    pub read_timeout: Duration,
    /// Dłuższy bok klatek, z jakim otwierana jest kamera (żądania dostają klatki pomniejszone).
    pub capture_side: u32,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            idle_sleep: Duration::from_secs(20),
            read_timeout: Duration::from_secs(5),
            capture_side: 1280,
        }
    }
}

struct Inner {
    port: Arc<dyn CameraPort>,
    store: Arc<dyn CameraStore>,
    config: HubConfig,
    registry: Mutex<Registry>,
    changes: watch::Sender<HubState>,
    /// Szereguje zapisy ustawień (ostatni zapis = ostatni stan).
    saving: tokio::sync::Mutex<()>,
}

impl Inner {
    /// Zamyka strumienie i dopiero po potwierdzeniu zamknięcia oznacza kamery jako uśpione.
    /// Stan „zamykana” jest ogłaszany od razu, żeby znak kamery nie zgasł przed czasem.
    async fn close_and_settle(&self, slots: Vec<(Arc<Slot>, u64)>) {
        if slots.is_empty() {
            return;
        }
        self.publish();
        close(slots.clone()).await;
        self.lock().settle(&slots);
    }

    fn lock(&self) -> MutexGuard<'_, Registry> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Ogłasza stan, jeśli się zmienił; zwraca bieżący.
    fn publish(&self) -> HubState {
        let state = self.lock().snapshot();
        self.changes.send_if_modified(|current| {
            let changed = *current != state;
            if changed {
                current.clone_from(&state);
            }
            changed
        });
        state
    }

    async fn save(&self) -> Result<(), CameraError> {
        let _order = self.saving.lock().await;
        let data = self.lock().stored.clone();
        // Chwilowy błąd zapisu (plik zajęty) nie może zostawić w ustawieniach „kamery włączone”
        // po tym, jak użytkownik je wyłączył — ponawiamy, zanim zgłosimy błąd.
        let mut last = String::new();
        for attempt in 0..SAVE_ATTEMPTS {
            if attempt > 0 {
                tokio::time::sleep(SAVE_RETRY_DELAY).await;
            }
            match self.store.save(&data).await {
                Ok(()) => return Ok(()),
                Err(text) => last = text,
            }
        }
        Err(CameraError::Store(last))
    }
}

/// Liczba prób zapisu ustawień kamer i odstęp między nimi.
const SAVE_ATTEMPTS: u32 = 3;
const SAVE_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(60);

/// Zamyka strumienie wskazanych gniazd (czeka na zakończenie odczytów w toku).
async fn close(slots: Vec<(Arc<Slot>, u64)>) {
    let tasks: Vec<_> = slots
        .into_iter()
        .map(|(slot, epoch)| tokio::task::spawn_blocking(move || slot.close_before(epoch)))
        .collect();
    for task in tasks {
        // Zadanie zamykające nie panikuje; przerwane (zamykanie środowiska) nie ma już czego bronić.
        let _ = task.await;
    }
}

/// Hub kamer (tani do klonowania).
#[derive(Clone)]
pub struct CameraHubService {
    inner: Arc<Inner>,
}

impl CameraHubService {
    /// Wczytuje ustawienia z magazynu. Urządzenia wylicza od razu tylko wtedy, gdy przełącznik
    /// główny jest włączony; w przeciwnym razie dopiero `refresh`.
    pub async fn start(
        port: Arc<dyn CameraPort>,
        store: Arc<dyn CameraStore>,
        config: HubConfig,
    ) -> Self {
        let registry = Registry::new(store.load().await);
        let master = registry.master;
        let (changes, _) = watch::channel(registry.snapshot());
        let hub = Self {
            inner: Arc::new(Inner {
                port,
                store,
                config,
                registry: Mutex::new(registry),
                changes,
                saving: tokio::sync::Mutex::new(()),
            }),
        };
        if master {
            // Błąd wyliczania nie blokuje startu: lista zostaje pusta do następnego `refresh`.
            let _ = hub.refresh().await;
        }
        hub
    }

    /// Zadanie usypiające: zamyka strumień po `idle_sleep` bez żądań. Kończy się, gdy kamera
    /// przestaje działać albo hub zostaje zwolniony.
    fn spawn_sleeper(inner: &Arc<Inner>, id: CameraId) {
        let weak: Weak<Inner> = Arc::downgrade(inner);
        let idle = inner.config.idle_sleep;
        tokio::spawn(async move {
            let mut wake = Instant::now() + idle;
            loop {
                tokio::time::sleep_until(wake).await;
                let Some(inner) = weak.upgrade() else {
                    return;
                };
                let closing = {
                    let mut registry = inner.lock();
                    let Ok(entry) = registry.entry(&id) else {
                        return;
                    };
                    if !entry.is_live() {
                        entry.sleeper = false;
                        return;
                    }
                    let due = entry.last_used + idle;
                    if entry.in_flight > 0 {
                        wake = Instant::now() + idle;
                        continue;
                    }
                    if due > Instant::now() {
                        wake = due;
                        continue;
                    }
                    entry.sleeper = false;
                    entry.stop()
                };
                inner.close_and_settle(vec![closing]).await;
                inner.publish();
                return;
            }
        });
    }

    /// Rozlicza zakończone żądanie klatki: stan kamery, usypianie, wynik dla wywołującego.
    fn finish(
        &self,
        id: &CameraId,
        epoch: u64,
        outcome: Result<(RgbaImage, (u32, u32)), ReadFailure>,
    ) -> Result<Frame, CameraError> {
        let result = {
            let mut registry = self.inner.lock();
            let entry = registry.entry(id)?;
            entry.in_flight = entry.in_flight.saturating_sub(1);
            entry.last_used = Instant::now();
            if entry.slot.epoch() != epoch {
                // Zatrzymanie wygrało: stan ustawił już zatrzymujący.
                Err(CameraError::Stopped)
            } else {
                match outcome {
                    Ok((image, (width, height))) => {
                        entry.state = CameraState::Live { width, height };
                        if !entry.sleeper {
                            entry.sleeper = true;
                            Self::spawn_sleeper(&self.inner, id.clone());
                        }
                        Ok(image)
                    }
                    Err(ReadFailure::Stopped) => {
                        entry.state = CameraState::Asleep;
                        Err(CameraError::Stopped)
                    }
                    Err(ReadFailure::Unplugged) => {
                        entry.present = false;
                        entry.state = CameraState::Unplugged;
                        Err(CameraError::Unplugged)
                    }
                    Err(ReadFailure::TimedOut(text)) => {
                        entry.state = CameraState::Failed(text);
                        Err(CameraError::Timeout)
                    }
                    Err(ReadFailure::Failed(text)) => {
                        entry.state = CameraState::Failed(text.clone());
                        Err(CameraError::Platform(text))
                    }
                }
            }
        };
        self.inner.publish();
        let taken_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        result.map(|image| Frame {
            camera: id.clone(),
            taken_ms,
            image,
        })
    }
}

#[async_trait]
impl CameraHub for CameraHubService {
    async fn state(&self) -> HubState {
        self.inner.lock().snapshot()
    }

    async fn set_master(&self, enabled: bool) -> Result<HubState, CameraError> {
        let closing = {
            let mut registry = self.inner.lock();
            if registry.master == enabled {
                return Ok(registry.snapshot());
            }
            registry.master = enabled;
            registry.stored.master = enabled;
            if enabled {
                Vec::new()
            } else {
                registry.cameras.iter_mut().map(|c| c.stop()).collect()
            }
        };
        self.inner.close_and_settle(closing).await;
        self.inner.publish();
        let saved = self.inner.save().await;
        if enabled {
            // Świeża lista urządzeń przy włączeniu; błąd wyliczania nie cofa przełącznika.
            let _ = self.refresh().await;
        }
        saved.map(|()| self.inner.lock().snapshot())
    }

    async fn set_enabled(&self, id: &CameraId, enabled: bool) -> Result<HubState, CameraError> {
        let closing = {
            let mut registry = self.inner.lock();
            let entry = registry.entry(id)?;
            if entry.enabled == enabled {
                return Ok(registry.snapshot());
            }
            entry.enabled = enabled;
            let closing = (!enabled).then(|| entry.stop());
            registry.remember(id);
            closing
        };
        self.inner
            .close_and_settle(closing.into_iter().collect())
            .await;
        let state = self.inner.publish();
        self.inner.save().await.map(|()| state)
    }

    async fn rename(&self, id: &CameraId, name: &str) -> Result<HubState, CameraError> {
        {
            let mut registry = self.inner.lock();
            let entry = registry.entry(id)?;
            let name = clean_name(name).filter(|n| *n != entry.system_name);
            if entry.custom_name == name {
                return Ok(registry.snapshot());
            }
            entry.custom_name = name;
            registry.remember(id);
        }
        let state = self.inner.publish();
        self.inner.save().await.map(|()| state)
    }

    async fn frame(&self, id: &CameraId, max_side: u32) -> Result<Frame, CameraError> {
        let (slot, epoch) = {
            let mut registry = self.inner.lock();
            if !registry.master {
                return Err(CameraError::Disabled);
            }
            let entry = registry.entry(id)?;
            if !entry.enabled {
                return Err(CameraError::Disabled);
            }
            if !entry.present {
                return Err(CameraError::Unplugged);
            }
            // Kamera już się otwiera dla innego żądania: nie ustawiamy kolejki zadań blokujących
            // (zawieszony sterownik zebrałby ich setki).
            if entry.in_flight > 0 && !entry.is_live() {
                return Err(CameraError::Busy);
            }
            entry.in_flight += 1;
            entry.last_used = Instant::now();
            if !entry.is_live() {
                entry.state = CameraState::Opening;
            }
            (entry.slot.clone(), entry.slot.epoch())
        };
        self.inner.publish();
        let plan = ReadPlan {
            epoch,
            capture_side: self.inner.config.capture_side,
            max_side: max_side.max(MIN_FRAME_SIDE),
            timeout: self.inner.config.read_timeout,
        };
        let (inner, link) = (self.inner.clone(), id.0.clone());
        let work = move || read_frame(inner.port.as_ref(), &slot, &link, plan);
        // Rozliczenie żądania działa w osobnym zadaniu: porzucenie tej przyszłości (np. przerwane
        // polecenie) nie zostawia licznika żądań w górze ani kamery bez usypiania.
        let (service, camera) = (self.clone(), id.clone());
        let settled = tokio::spawn(async move {
            let outcome = tokio::task::spawn_blocking(work).await.unwrap_or_else(|e| {
                Err(ReadFailure::Failed(format!(
                    "zadanie kamery przerwane: {e}"
                )))
            });
            service.finish(&camera, epoch, outcome)
        });
        settled.await.unwrap_or_else(|e| {
            Err(CameraError::Platform(format!(
                "zadanie kamery przerwane: {e}"
            )))
        })
    }

    async fn stop_all(&self) {
        let closing = {
            let mut registry = self.inner.lock();
            registry.cameras.iter_mut().map(|c| c.stop()).collect()
        };
        self.inner.close_and_settle(closing).await;
        self.inner.publish();
    }

    async fn refresh(&self) -> Result<HubState, CameraError> {
        let port = self.inner.port.clone();
        let devices = tokio::task::spawn_blocking(move || port.devices())
            .await
            .map_err(|e| CameraError::Platform(format!("wyliczanie kamer przerwane: {e}")))?
            .map_err(|e| CameraError::Platform(e.to_string()))?;
        let closing = self.inner.lock().merge(devices);
        self.inner.close_and_settle(closing).await;
        Ok(self.inner.publish())
    }

    fn changes(&self) -> watch::Receiver<HubState> {
        self.inner.changes.subscribe()
    }
}
