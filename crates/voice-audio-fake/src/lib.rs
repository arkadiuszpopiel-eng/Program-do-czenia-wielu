//! Atrapa `voice-audio` (docs/modules/voice-audio/SPEC.md §Fake): wirtualne urządzenia z wirtualnym
//! zegarem. Mikrofon odtwarza WAV / wygenerowane próbki (+ opcjonalne echo wyjścia przez
//! [`EchoPath`] — pętla zwrotna do testów AEC i barge-in), wyjście renderuje ten sam mikser co
//! `-impl` i zapisuje wynik do bufora. Hot-plug, konflikt trybu wyłącznego i opóźnienie urządzenia
//! są symulowane. Czas płynie wyłącznie przez [`FakeAudio::advance`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod device;
mod echo;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

pub use echo::EchoPath;
use voice_audio_contract::{
    AudioDevice, AudioError, AudioFormat, AudioIo, CaptureWriter, DeviceEvent, DeviceId,
    DeviceKind, DeviceStatus, InputStream, MediaClock, MediaTime, MixerConfig, MixerOutput,
    MixerRender, OutputStream, Resampler, StreamConfig, capture_ring, mixer::mixer, wav,
};

/// Częstotliwość wirtualnych urządzeń.
pub const FAKE_RATE: u32 = 48_000;
/// Okres urządzenia (jeden „callback”).
pub const PERIOD: Duration = Duration::from_millis(10);
const PERIOD_SAMPLES: usize = (FAKE_RATE / 100) as usize;
/// Historia wyjścia potrzebna do echa (2 s).
const HISTORY: usize = FAKE_RATE as usize * 2;

struct OutDev {
    device: DeviceId,
    render: MixerRender,
    status: Arc<DeviceStatus>,
    channels: u16,
}

struct InDev {
    device: DeviceId,
    writer: CaptureWriter,
    channels: u16,
    loopback: bool,
}

#[derive(Default)]
struct Inner {
    now: MediaTime,
    devices: Vec<AudioDevice>,
    events: VecDeque<DeviceEvent>,
    outputs: Vec<OutDev>,
    inputs: Vec<InDev>,
    mic: Vec<f32>,
    mic_pos: usize,
    mic_loop: bool,
    echo: Option<EchoPath>,
    /// Niezerowe współczynniki echa (indeks, wartość) — rzadki „pokój” liczony szybko.
    echo_taps: Vec<(usize, f32)>,
    output_latency: Duration,
    /// Zmiksowane wyjście (mono) — całe od startu (do asercji w testach).
    recorded: Vec<f32>,
    /// Historia wyjścia na potrzeby echa (indeks absolutny = `history_start + i`).
    history: VecDeque<f32>,
    history_start: u64,
    fail_next_open: Option<AudioError>,
}

/// Wirtualne audio (klonowanie = wspólny stan).
#[derive(Clone)]
pub struct FakeAudio {
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for FakeAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeAudio")
            .field("now", &self.lock().now)
            .finish()
    }
}

impl Default for FakeAudio {
    fn default() -> Self {
        Self::new()
    }
}

fn device(id: &str, name: &str, kind: DeviceKind, is_default: bool) -> AudioDevice {
    AudioDevice {
        id: DeviceId::new(id),
        name: name.into(),
        kind,
        is_default,
        bluetooth: voice_audio_contract::looks_like_bluetooth(name),
        mix_format: Some(AudioFormat::stereo(FAKE_RATE)),
    }
}

impl FakeAudio {
    /// Mikrofon i głośniki wirtualne (48 kHz), opóźnienie wyjścia 20 ms, bez echa.
    pub fn new() -> Self {
        let inner = Inner {
            devices: vec![
                device("mic-0", "Mikrofon (wirtualny)", DeviceKind::Input, true),
                device("spk-0", "Głośniki (wirtualne)", DeviceKind::Output, true),
            ],
            output_latency: Duration::from_millis(20),
            ..Inner::default()
        };
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Bieżący czas wirtualny.
    pub fn now(&self) -> MediaTime {
        self.lock().now
    }

    /// Sygnał mikrofonu (mono, dowolna obsługiwana częstotliwość → 48 kHz); `looped` = w kółko.
    pub fn set_mic_signal(&self, samples: &[f32], rate: u32, looped: bool) {
        let s = Resampler::convert(rate, FAKE_RATE, samples);
        let mut g = self.lock();
        g.mic = s;
        g.mic_pos = 0;
        g.mic_loop = looped;
    }

    /// Mikrofon z pliku WAV (miksowany do mono).
    pub fn load_wav(&self, bytes: &[u8]) -> Result<(), AudioError> {
        let (pcm, format) = wav::decode_wav(bytes)?;
        let mono = voice_audio_contract::downmix(&pcm, format.channels);
        self.set_mic_signal(&mono, format.sample_rate, false);
        Ok(())
    }

    /// Ścieżka echa głośnik → mikrofon (`None` = słuchawki, brak echa).
    pub fn set_echo(&self, echo: Option<EchoPath>) {
        let mut g = self.lock();
        g.echo_taps = echo.as_ref().map_or_else(Vec::new, |e| {
            e.taps
                .iter()
                .enumerate()
                .filter(|(_, t)| **t != 0.0)
                .map(|(k, t)| (k, *t))
                .collect()
        });
        g.echo = echo;
    }

    /// Opóźnienie wyjścia urządzenia (bufor + sprzęt).
    pub fn set_output_latency(&self, latency: Duration) {
        let mut g = self.lock();
        g.output_latency = latency;
        for o in &g.outputs {
            o.status.set_output_latency(latency);
        }
    }

    /// Całe wyrenderowane wyjście (mono, 48 kHz).
    pub fn recorded_output(&self) -> Vec<f32> {
        self.lock().recorded.clone()
    }

    /// Liczba próbek wyrenderowanego wyjścia (bez kopiowania nagrania).
    pub fn recorded_len(&self) -> usize {
        self.lock().recorded.len()
    }

    /// Fragment wyrenderowanego wyjścia `[from, to)` (indeksy próbek 48 kHz, przycięte do nagrania).
    pub fn recorded_range(&self, from: usize, to: usize) -> Vec<f32> {
        let g = self.lock();
        let to = to.min(g.recorded.len());
        g.recorded
            .get(from.min(to)..to)
            .map(<[f32]>::to_vec)
            .unwrap_or_default()
    }

    /// Liczba otwartych (niezamkniętych przez konsumenta) strumieni mikrofonu — bez pętli zwrotnej.
    pub fn open_inputs(&self) -> usize {
        self.lock()
            .inputs
            .iter()
            .filter(|i| !i.loopback && !i.writer.is_abandoned())
            .count()
    }

    /// Następne `open_*` zwróci błąd (np. `ExclusiveConflict`, `PermissionDenied`).
    pub fn fail_next_open(&self, error: AudioError) {
        self.lock().fail_next_open = Some(error);
    }

    /// Podłącza urządzenie (hot-plug).
    pub fn plug(&self, id: &str, name: &str, kind: DeviceKind) {
        let d = device(id, name, kind, false);
        let mut g = self.lock();
        g.devices.push(d.clone());
        g.events.push_back(DeviceEvent::Added { device: d });
    }

    /// Ustawia domyślne urządzenie kierunku.
    pub fn set_default(&self, id: &str) {
        let mut g = self.lock();
        let Some(kind) = g
            .devices
            .iter()
            .find(|d| d.id.as_str() == id)
            .map(|d| d.kind)
        else {
            return;
        };
        for d in g.devices.iter_mut().filter(|d| d.kind == kind) {
            d.is_default = d.id.as_str() == id;
        }
        g.events.push_back(DeviceEvent::DefaultChanged {
            kind,
            id: Some(DeviceId::new(id)),
        });
    }

    /// Odłącza urządzenie: otwarte strumienie są zamykane (`AudioError::Closed`), domyślne
    /// przechodzi na pierwsze pozostałe urządzenie kierunku.
    pub fn unplug(&self, id: &str) {
        let mut g = self.lock();
        let Some(idx) = g.devices.iter().position(|d| d.id.as_str() == id) else {
            return;
        };
        let removed = g.devices.remove(idx);
        g.events.push_back(DeviceEvent::Removed {
            id: removed.id.clone(),
        });
        for o in g.outputs.iter().filter(|o| o.device == removed.id) {
            o.status
                .closed
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        g.outputs.retain(|o| o.device != removed.id);
        g.inputs.retain(|i| i.device != removed.id);
        if removed.is_default {
            let next = g.devices.iter_mut().find(|d| d.kind == removed.kind);
            let id = next.map(|d| {
                d.is_default = true;
                d.id.clone()
            });
            g.events.push_back(DeviceEvent::DefaultChanged {
                kind: removed.kind,
                id,
            });
        }
    }

    /// Upływ czasu wirtualnego: renderuje wyjścia i generuje wejścia okresami po 10 ms.
    pub fn advance(&self, d: Duration) {
        let periods = d.as_nanos().div_ceil(PERIOD.as_nanos()) as usize;
        let mut g = self.lock();
        for _ in 0..periods {
            g.step();
        }
    }
}

impl FakeAudio {
    fn open_capture(
        &self,
        device: Option<&DeviceId>,
        config: &StreamConfig,
        loopback: bool,
    ) -> Result<Box<dyn InputStream>, AudioError> {
        config.validate()?;
        if config.format.sample_rate != FAKE_RATE {
            return Err(AudioError::Format("atrapa: wejście tylko 48 kHz".into()));
        }
        let kind = if loopback {
            DeviceKind::Output
        } else {
            DeviceKind::Input
        };
        let mut g = self.lock();
        let id = g.check_open(device, kind)?;
        let (writer, reader) = capture_ring(config.format, config.frame_ms, config.queue_ms);
        g.inputs.push(InDev {
            device: id,
            writer,
            channels: config.format.channels,
            loopback,
        });
        Ok(Box::new(reader))
    }
}

impl MediaClock for FakeAudio {
    fn now(&self) -> MediaTime {
        self.lock().now
    }
}

impl AudioIo for FakeAudio {
    fn devices(&self) -> Result<Vec<AudioDevice>, AudioError> {
        Ok(self.lock().devices.clone())
    }

    fn poll_device_events(&self) -> Vec<DeviceEvent> {
        self.lock().events.drain(..).collect()
    }

    fn open_input(
        &self,
        device: Option<&DeviceId>,
        config: &StreamConfig,
    ) -> Result<Box<dyn InputStream>, AudioError> {
        self.open_capture(device, config, false)
    }

    fn open_output(
        &self,
        device: Option<&DeviceId>,
        config: &StreamConfig,
    ) -> Result<Box<dyn OutputStream>, AudioError> {
        config.validate()?;
        let mut g = self.lock();
        let id = g.check_open(device, DeviceKind::Output)?;
        let format = AudioFormat {
            sample_rate: FAKE_RATE,
            channels: config.format.channels,
        };
        let (control, render) = mixer(MixerConfig::new(FAKE_RATE));
        let status = Arc::new(DeviceStatus::default());
        status.set_output_latency(g.output_latency);
        g.outputs.push(OutDev {
            device: id,
            render,
            status: Arc::clone(&status),
            channels: format.channels,
        });
        Ok(Box::new(MixerOutput::new(control, format, status)))
    }

    fn open_loopback(
        &self,
        device: Option<&DeviceId>,
        config: &StreamConfig,
    ) -> Result<Box<dyn InputStream>, AudioError> {
        self.open_capture(device, config, true)
    }
}
