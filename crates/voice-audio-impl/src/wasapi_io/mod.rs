//! WASAPI przez crate `wasapi` (Windows): urządzenia MMDevice, hot-plug (`IMMNotificationClient`
//! na własnym wątku), strumienie w trybie współdzielonym sterowane zdarzeniami (`stream.rs`).
//! Obiekty COM nie opuszczają wątków, które je utworzyły (typy `wasapi` nie są `Send`).

mod stream;

use std::collections::VecDeque;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use voice_audio_contract::{
    AudioDevice, AudioError, AudioFormat, AudioIo, DeviceEvent, DeviceId, DeviceKind, InputStream,
    OutputStream, StreamConfig, looks_like_bluetooth,
};
use wasapi::{DeviceEnumerator, DeviceEventCallbacks, Direction, WasapiError};

pub(crate) use stream::{CaptureKind, open_capture, open_render};

/// HRESULT-y mapowane na błędy kontraktu.
const AUDCLNT_E_DEVICE_INVALIDATED: u32 = 0x8889_0004;
const AUDCLNT_E_DEVICE_IN_USE: u32 = 0x8889_000A;
const AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED: u32 = 0x8889_000E;
const E_ACCESSDENIED: u32 = 0x8007_0005;
const E_NOTFOUND: u32 = 0x8007_0490;

/// Mapuje błąd `wasapi` na błąd kontraktu.
pub(crate) fn map_err(e: WasapiError, device: &str) -> AudioError {
    match &e {
        WasapiError::Windows(w) => match w.code().0 as u32 {
            AUDCLNT_E_DEVICE_IN_USE | AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED => {
                AudioError::ExclusiveConflict(device.to_owned())
            }
            E_ACCESSDENIED => AudioError::PermissionDenied,
            AUDCLNT_E_DEVICE_INVALIDATED => AudioError::Closed,
            E_NOTFOUND => AudioError::DeviceNotFound(device.to_owned()),
            _ => AudioError::Backend(e.to_string()),
        },
        WasapiError::DeviceNotFound(name) => AudioError::DeviceNotFound(name.clone()),
        WasapiError::UnsupportedFormat => AudioError::Format(e.to_string()),
        _ => AudioError::Backend(e.to_string()),
    }
}

fn direction(kind: DeviceKind) -> Direction {
    match kind {
        DeviceKind::Input => Direction::Capture,
        DeviceKind::Output => Direction::Render,
    }
}

/// Inicjuje COM (MTA) w bieżącym wątku; ponowne wywołanie jest nieszkodliwe.
pub(crate) fn com_init() {
    let _ = wasapi::initialize_mta();
}

fn list(kind: DeviceKind) -> Result<Vec<AudioDevice>, AudioError> {
    com_init();
    let enumerator = DeviceEnumerator::new().map_err(|e| map_err(e, "enumerator"))?;
    let default_id = enumerator
        .get_default_device(&direction(kind))
        .and_then(|d| d.get_id())
        .ok();
    let collection = enumerator
        .get_device_collection(&direction(kind))
        .map_err(|e| map_err(e, "kolekcja"))?;
    let count = collection
        .get_nbr_devices()
        .map_err(|e| map_err(e, "kolekcja"))?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let Ok(device) = collection.get_device_at_index(i) else {
            continue;
        };
        let Ok(id) = device.get_id() else {
            continue;
        };
        let name = device.get_friendlyname().unwrap_or_else(|_| id.clone());
        let mix_format = device.get_device_format().ok().map(|f| AudioFormat {
            sample_rate: f.get_samplespersec(),
            channels: f.get_nchannels(),
        });
        out.push(AudioDevice {
            is_default: default_id.as_deref() == Some(id.as_str()),
            bluetooth: looks_like_bluetooth(&name)
                || id.contains("BTHENUM")
                || id.contains("BTHHFENUM"),
            id: DeviceId::new(id),
            name,
            kind,
            mix_format,
        });
    }
    Ok(out)
}

/// Wątek powiadomień o urządzeniach (rejestracja COM żyje tylko w nim).
struct Notifier {
    stop: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Notifier {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Surowe powiadomienie z wątku systemowego (uzupełniane w `poll_device_events`).
#[derive(Debug, Clone)]
enum RawEvent {
    Added(String),
    Removed(String),
    Default(DeviceKind, Option<String>),
}

type EventQueue = Arc<Mutex<VecDeque<RawEvent>>>;

fn lock(q: &EventQueue) -> MutexGuard<'_, VecDeque<RawEvent>> {
    q.lock().unwrap_or_else(|p| p.into_inner())
}

fn spawn_notifier(queue: EventQueue) -> Option<Notifier> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let thread = std::thread::Builder::new()
        .name("alfa-audio-notify".into())
        .spawn(move || {
            com_init();
            let Ok(enumerator) = DeviceEnumerator::new() else {
                return;
            };
            let mut cb = DeviceEventCallbacks::new();
            let q = Arc::clone(&queue);
            cb.set_device_added_callback(move |id| lock(&q).push_back(RawEvent::Added(id)));
            let q = Arc::clone(&queue);
            cb.set_device_removed_callback(move |id| lock(&q).push_back(RawEvent::Removed(id)));
            let q = Arc::clone(&queue);
            cb.set_default_device_callback(move |dir, role, id| {
                if matches!(role, wasapi::Role::Console) {
                    let kind = match dir {
                        Direction::Capture => DeviceKind::Input,
                        Direction::Render => DeviceKind::Output,
                    };
                    lock(&q).push_back(RawEvent::Default(kind, id));
                }
            });
            let Ok(_registration) = enumerator.register_notification_callback(cb) else {
                return;
            };
            let _ = stop_rx.recv();
        })
        .ok()?;
    Some(Notifier {
        stop: stop_tx,
        thread: Some(thread),
    })
}

/// Audio Windows (WASAPI, tryb współdzielony, zdarzeniowy).
pub struct WasapiAudio {
    events: EventQueue,
    _notifier: Option<Notifier>,
}

impl std::fmt::Debug for WasapiAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasapiAudio").finish_non_exhaustive()
    }
}

impl Default for WasapiAudio {
    fn default() -> Self {
        Self::new()
    }
}

impl WasapiAudio {
    /// Uruchamia wątek powiadomień o urządzeniach.
    pub fn new() -> Self {
        let events: EventQueue = Arc::default();
        let notifier = spawn_notifier(Arc::clone(&events));
        Self {
            events,
            _notifier: notifier,
        }
    }
}

impl AudioIo for WasapiAudio {
    fn devices(&self) -> Result<Vec<AudioDevice>, AudioError> {
        let mut all = list(DeviceKind::Input)?;
        all.extend(list(DeviceKind::Output)?);
        Ok(all)
    }

    fn poll_device_events(&self) -> Vec<DeviceEvent> {
        let raw: Vec<RawEvent> = lock(&self.events).drain(..).collect();
        if raw.is_empty() {
            return Vec::new();
        }
        // Szczegóły dodanego urządzenia (nazwa, kierunek) z ponownego wyliczenia.
        let known = self.devices().unwrap_or_default();
        raw.into_iter()
            .filter_map(|e| match e {
                RawEvent::Added(id) => known
                    .iter()
                    .find(|d| d.id.as_str() == id)
                    .cloned()
                    .map(|device| DeviceEvent::Added { device }),
                RawEvent::Removed(id) => Some(DeviceEvent::Removed {
                    id: DeviceId::new(id),
                }),
                RawEvent::Default(kind, id) => Some(DeviceEvent::DefaultChanged {
                    kind,
                    id: id.map(DeviceId::new),
                }),
            })
            .collect()
    }

    fn open_input(
        &self,
        device: Option<&DeviceId>,
        config: &StreamConfig,
    ) -> Result<Box<dyn InputStream>, AudioError> {
        config.validate()?;
        open_capture(device.cloned(), *config, CaptureKind::Microphone)
    }

    fn open_output(
        &self,
        device: Option<&DeviceId>,
        config: &StreamConfig,
    ) -> Result<Box<dyn OutputStream>, AudioError> {
        config.validate()?;
        open_render(device.cloned(), *config)
    }

    fn open_loopback(
        &self,
        device: Option<&DeviceId>,
        config: &StreamConfig,
    ) -> Result<Box<dyn InputStream>, AudioError> {
        config.validate()?;
        open_capture(device.cloned(), *config, CaptureKind::Loopback)
    }
}

/// Czas oczekiwania na zdarzenie urządzenia, po którym wątek sprawdza flagę zatrzymania.
pub(crate) const EVENT_WAIT: Duration = Duration::from_millis(200);
