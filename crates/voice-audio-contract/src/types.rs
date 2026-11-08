//! Typy kontraktu: urządzenia, konfiguracja strumieni, źródła miksera, opóźnienia, błędy, zdarzenia.

use std::fmt;
use std::time::Duration;

use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::frame::{AudioFormat, MediaTime};

/// Identyfikator urządzenia (Windows: identyfikator punktu końcowego MMDevice).
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct DeviceId(pub String);

impl DeviceId {
    /// Nowy identyfikator.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Kierunek urządzenia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    /// Mikrofon.
    Input,
    /// Głośniki / słuchawki.
    Output,
}

/// Urządzenie audio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AudioDevice {
    /// Identyfikator.
    pub id: DeviceId,
    /// Nazwa przyjazna.
    pub name: String,
    /// Kierunek.
    pub kind: DeviceKind,
    /// Domyślne urządzenie systemu dla kierunku.
    pub is_default: bool,
    /// Bluetooth (HFP obniża jakość do 16 kHz i dodaje 150–300 ms → ostrzeżenie w UI).
    pub bluetooth: bool,
    /// Format miksu systemowego (tryb współdzielony), jeśli znany.
    pub mix_format: Option<AudioFormat>,
}

/// Zmiana urządzeń (hot-plug, zmiana domyślnego) — zdarzenie `voice.audio.device.changed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum DeviceEvent {
    /// Podłączono urządzenie.
    Added {
        /// Urządzenie.
        device: AudioDevice,
    },
    /// Odłączono urządzenie.
    Removed {
        /// Identyfikator.
        id: DeviceId,
    },
    /// Zmiana domyślnego urządzenia kierunku (`None` = brak domyślnego).
    DefaultChanged {
        /// Kierunek.
        kind: DeviceKind,
        /// Nowe domyślne.
        id: Option<DeviceId>,
    },
}

/// Konfiguracja strumienia (`[machine.audio]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StreamConfig {
    /// Format żądany od urządzenia (tryb współdzielony konwertuje automatycznie).
    pub format: AudioFormat,
    /// Długość ramki wejściowej (10–20 ms).
    pub frame_ms: u32,
    /// Tryb wyłączny (domyślnie wyłączony; konflikt → `AudioError::ExclusiveConflict`).
    pub exclusive: bool,
    /// Pojemność kolejki (ms) między wątkiem RT a konsumentem.
    pub queue_ms: u32,
}

impl StreamConfig {
    /// Domyślne wejście: 48 kHz mono, ramki 10 ms.
    pub const fn input_default() -> Self {
        Self {
            format: AudioFormat::mono(48_000),
            frame_ms: 10,
            exclusive: false,
            queue_ms: 2_000,
        }
    }

    /// Domyślne wyjście: 48 kHz stereo, kolejka TTS 30 s.
    pub const fn output_default() -> Self {
        Self {
            format: AudioFormat::stereo(48_000),
            frame_ms: 10,
            exclusive: false,
            queue_ms: 30_000,
        }
    }

    /// Walidacja (format, ramka 5–40 ms, kolejka ≥ 100 ms).
    pub fn validate(&self) -> Result<(), AudioError> {
        self.format.validate()?;
        if !(5..=40).contains(&self.frame_ms) {
            return Err(AudioError::Format(format!(
                "ramka {} ms poza zakresem 5–40 ms",
                self.frame_ms
            )));
        }
        if self.queue_ms < 100 {
            return Err(AudioError::Format("kolejka krótsza niż 100 ms".into()));
        }
        Ok(())
    }
}

/// Źródło dźwięku w mikserze (routing per agentka).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "persona", rename_all = "snake_case")]
pub enum SourceId {
    /// Mowa agentki (TTS) — tor głosu, liczy się do „usłyszanego prefiksu”.
    Tts(PersonaId),
    /// Filler agentki („hmm”, „już sprawdzam”) — tor głosu, poza prefiksem, przerywalny.
    Filler(PersonaId),
    /// Earcon (dźwięk stanu) — tor efektów, miksowany na głos.
    Earcon,
}

impl SourceId {
    /// Tor miksera źródła.
    pub fn lane(&self) -> Lane {
        match self {
            SourceId::Tts(_) | SourceId::Filler(_) => Lane::Voice,
            SourceId::Earcon => Lane::Effects,
        }
    }

    /// Agentka źródła (jeśli dotyczy).
    pub fn persona(&self) -> Option<&PersonaId> {
        match self {
            SourceId::Tts(p) | SourceId::Filler(p) => Some(p),
            SourceId::Earcon => None,
        }
    }
}

/// Tor miksera. Tor głosu odtwarza **jedną** wypowiedź naraz (nie miesza dwóch TTS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    /// Głos agentki (TTS, fillery).
    Voice,
    /// Efekty (earcony).
    Effects,
}

/// Wyciszanie głosu agentki, gdy mówi użytkownik (zatrzymanie dwustopniowe, krok 1).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Ducking {
    /// Tłumienie w dB (ujemne; domyślnie −15 dB).
    pub gain_db: f32,
    /// Czas rampy (≤ 50 ms).
    pub attack: Duration,
}

impl Default for Ducking {
    fn default() -> Self {
        Self {
            gain_db: -15.0,
            attack: Duration::from_millis(20),
        }
    }
}

impl Ducking {
    /// Najdłuższa dozwolona rampa (SPEC: ducking ≤ 50 ms).
    pub const MAX_ATTACK: Duration = Duration::from_millis(50);

    /// Walidacja: tłumienie w [−60, 0] dB, rampa ≤ 50 ms.
    pub fn validate(&self) -> Result<(), AudioError> {
        if !(-60.0..=0.0).contains(&self.gain_db) {
            return Err(AudioError::Format(format!(
                "tłumienie {} dB poza [−60, 0]",
                self.gain_db
            )));
        }
        if self.attack > Self::MAX_ATTACK {
            return Err(AudioError::Format(
                "rampa duckingu dłuższa niż 50 ms".into(),
            ));
        }
        Ok(())
    }
}

/// Opóźnienia pętli audio (heard prefix, AEC).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LoopLatency {
    /// Opóźnienie wyjścia: od wyrenderowania próbki do przetwornika (bufor + sprzęt).
    pub output: Duration,
    /// Opóźnienie wejścia: od mikrofonu do dostarczenia ramki.
    pub input: Duration,
    /// Skalibrowane opóźnienie pętli głośnik → mikrofon (sygnał testowy), jeśli zmierzone.
    pub calibrated_loop: Option<Duration>,
}

/// Pozycja odtwarzania wypowiedzi (licznik próbek skorygowany o opóźnienie urządzenia).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlaybackPosition {
    /// Wypowiedź.
    pub utterance: u64,
    /// Próbki (na kanał, częstotliwość urządzenia) wyrenderowane do urządzenia.
    pub rendered_samples: u64,
    /// Próbki wypowiedzi przekazane do miksera.
    pub queued_samples: u64,
    /// Częstotliwość urządzenia.
    pub sample_rate: u32,
    /// Opóźnienie wyjścia w chwili odczytu.
    pub output_latency: Duration,
    /// Stan wypowiedzi.
    pub state: PlaybackState,
}

impl PlaybackPosition {
    /// Próbki, które zdążyły wybrzmieć (wyrenderowane minus opóźnienie wyjścia).
    pub fn heard_samples(&self) -> u64 {
        let lat = MediaTime(u64::try_from(self.output_latency.as_nanos()).unwrap_or(u64::MAX))
            .to_samples(self.sample_rate);
        match self.state {
            PlaybackState::Finished => self.rendered_samples,
            _ => self.rendered_samples.saturating_sub(lat),
        }
    }

    /// Czas, który wybrzmiał.
    pub fn heard(&self) -> Duration {
        Duration::from_nanos(MediaTime::from_samples(self.heard_samples(), self.sample_rate).0)
    }
}

/// Stan wypowiedzi w torze głosu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    /// W kolejce (jeszcze nie gra).
    Queued,
    /// Gra.
    Playing,
    /// Wybrzmiała do końca.
    Finished,
    /// Przerwana (`stop_all`).
    Stopped,
}

/// Kierunek strumienia (zdarzenia).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StreamDirection {
    /// Wejście (mikrofon).
    Input,
    /// Wyjście.
    Output,
    /// Pętla zwrotna wyjścia (referencja awaryjna AEC).
    Loopback,
}

/// Błędy modułu audio.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum AudioError {
    /// Platforma bez implementacji (np. `-impl` poza Windows).
    #[error("audio nieobsługiwane na tej platformie: {0}")]
    Unsupported(String),
    /// Nie ma takiego urządzenia (odłączone?).
    #[error("nie znaleziono urządzenia: {0}")]
    DeviceNotFound(String),
    /// Brak domyślnego urządzenia kierunku.
    #[error("brak domyślnego urządzenia")]
    NoDefaultDevice,
    /// Urządzenie zajęte przez aplikację w trybie wyłącznym.
    #[error("urządzenie zajęte w trybie wyłącznym przez inną aplikację: {0}")]
    ExclusiveConflict(String),
    /// Brak zgody systemu na mikrofon (`ms-settings:privacy-microphone`).
    #[error("brak dostępu do mikrofonu (Ustawienia → Prywatność → Mikrofon)")]
    PermissionDenied,
    /// Niepoprawny format / parametr.
    #[error("niepoprawny format: {0}")]
    Format(String),
    /// Kolejka pełna (odtwarzanie nie nadąża).
    #[error("kolejka audio pełna")]
    QueueFull,
    /// Tor głosu zajęty przez wypowiedź innej agentki (mikser nie miesza dwóch TTS).
    #[error("tor głosu zajęty przez inną wypowiedź ({playing})")]
    VoiceBusy {
        /// Wypowiedź, która gra.
        playing: u64,
    },
    /// Strumień zamknięty (np. wątek RT zakończony po odłączeniu urządzenia).
    #[error("strumień zamknięty")]
    Closed,
    /// Błąd warstwy systemowej.
    #[error("błąd audio: {0}")]
    Backend(String),
}
