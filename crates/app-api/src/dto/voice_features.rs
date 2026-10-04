//! Głos rozszerzony F5 (`voice_features`, `voice_wake`, `voice_speaker`, `voice_dictation`,
//! `voice_read`; zdarzenie `VoiceFeaturesChanged`) — odpowiednik `types-voice.ts`. Widok nigdy
//! nie zawiera audio, embeddingu mówcy ani czytanego tekstu; podgląd dyktowania to ostatnia fraza
//! (tylko w oknie Alfy, nie na magistrali i nie w logach).

use serde::{Deserialize, Serialize};

use super::common::LocalizedText;

/// Stan słów wywoławczych.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeWordsState {
    Unavailable,
    Off,
    Armed,
    Listening,
    Suspended,
}

/// Wynik pomiaru FAR/FRR z runnera `alfa-wake-eval` (brak pomiaru — `measured = false`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WakeCalibrationView {
    pub measured: bool,
    pub sufficient: bool,
    pub passes: bool,
    pub far_per_day: Option<f64>,
    pub frr: Option<f64>,
    pub threshold: f64,
}

/// Test słowa wywoławczego (wykrycia bez otwierania rozmowy).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WakeTestView {
    pub active: bool,
    pub detections: u32,
    pub last_agent: Option<String>,
    pub owner_rejected: u32,
}

/// Słowa wywoławcze „Hej Alfa/Beta/Gama/Delta”.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WakeWordsView {
    pub state: WakeWordsState,
    pub reason: Option<LocalizedText>,
    pub enabled: bool,
    pub risk_accepted: bool,
    pub owner_gate: bool,
    pub dnd: bool,
    pub phrases: Vec<String>,
    pub calibration: WakeCalibrationView,
    pub test: WakeTestView,
}

/// Stan rejestracji głosu właściciela.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerState {
    Unavailable,
    NotEnrolled,
    Enrolling,
    Enrolled,
}

/// Jakość nagranej frazy rejestracji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleQuality {
    Good,
    TooShort,
    TooQuiet,
    Inconsistent,
    Failed,
}

/// Ostatnia nagrana fraza rejestracji (bez audio).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnrollSampleView {
    pub accepted: bool,
    pub quality: SampleQuality,
    pub level_db: f64,
    pub duration_ms: u32,
    pub message: Option<LocalizedText>,
}

/// Decyzja weryfikacji mówcy ostatniej tury głosowej.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerDecisionView {
    Verified,
    Likely,
    Rejected,
    NotChecked,
}

/// Wynik weryfikacji ostatniej tury głosowej (wynik w ‰, bez embeddingu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeakerCheckView {
    pub decision: SpeakerDecisionView,
    pub score_permille: Option<u16>,
}

/// Weryfikacja właściciela („Rozpoznawanie mojego głosu”).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeakerView {
    pub state: SpeakerState,
    pub reason: Option<LocalizedText>,
    pub done: u32,
    pub needed: u32,
    pub recording: bool,
    pub prompts: Vec<LocalizedText>,
    pub last_sample: Option<EnrollSampleView>,
    pub required_for_risky: bool,
    pub last_check: Option<SpeakerCheckView>,
}

/// Stan dyktowania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DictationStateView {
    Unavailable,
    Idle,
    Active,
    Paused,
}

/// Profil dyktowania dla aplikacji (nazwa pliku procesu, np. `notepad.exe`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DictationProfile {
    pub app: String,
    pub capitalize_start: bool,
    pub block_enter: bool,
}

/// Dyktowanie do dowolnej aplikacji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DictationView {
    pub state: DictationStateView,
    pub reason: Option<LocalizedText>,
    pub app: Option<String>,
    pub typed_chars: u64,
    pub pending_chars: u64,
    pub can_undo: bool,
    pub preview: Option<String>,
    pub shortcut: String,
    pub profiles: Vec<DictationProfile>,
}

/// Stan czytania na głos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadStateView {
    Unavailable,
    Idle,
    Speaking,
    Paused,
}

/// Czytanie zaznaczenia / dokumentu / schowka głosem bieżącej agentki.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadAloudView {
    pub state: ReadStateView,
    pub reason: Option<LocalizedText>,
    pub app: Option<String>,
    pub index: u32,
    pub segments: u32,
    pub rate: f64,
    pub queued: u32,
    pub agent: String,
    pub shortcut: String,
}

/// Szybka rozmowa w chmurze (speech-to-speech) — dziś tylko kontrakt i atrapa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct S2sView {
    pub available: bool,
    pub reason: LocalizedText,
}

/// Głos rozszerzony (panel Głos, Ustawienia → Głos).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceFeatures {
    pub wake: WakeWordsView,
    pub speaker: SpeakerView,
    pub dictation: DictationView,
    pub read: ReadAloudView,
    pub s2s: S2sView,
}

/// Akcja słów wywoławczych.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WakeAction {
    /// Włączenie wyłącznie jawne; bez pomiaru FAR/FRR wymaga `accept_risk`.
    Configure {
        enabled: bool,
        accept_risk: bool,
        owner_gate: bool,
    },
    Test {
        on: bool,
    },
    SetDnd {
        on: bool,
    },
}

/// Akcja kreatora rejestracji i weryfikacji właściciela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpeakerAction {
    Begin,
    RecordStart,
    RecordStop,
    Finish,
    Cancel,
    Delete,
    SetRequired { required: bool },
}

/// Akcja dyktowania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DictationAction {
    Start,
    Stop,
    Toggle,
    Undo,
    SaveProfile { profile: DictationProfile },
    RemoveProfile { app: String },
}

/// Skąd czytać.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadSource {
    Selection,
    Document,
    Clipboard,
}

/// Sterowanie czytaniem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadControlAction {
    Pause,
    Resume,
    Next,
    Previous,
    Faster,
    Slower,
    Restart,
    Stop,
}

/// Akcja czytania na głos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ReadAction {
    Start { source: ReadSource },
    Control { control: ReadControlAction },
    SetRate { rate: f64 },
}

impl VoiceFeatures {
    /// Wszystko niedostępne (głos niepodłączony) z jednym powodem.
    pub fn unavailable(reason: LocalizedText) -> Self {
        Self {
            wake: WakeWordsView {
                state: WakeWordsState::Unavailable,
                reason: Some(reason.clone()),
                enabled: false,
                risk_accepted: false,
                owner_gate: true,
                dnd: false,
                phrases: Vec::new(),
                calibration: WakeCalibrationView {
                    measured: false,
                    sufficient: false,
                    passes: false,
                    far_per_day: None,
                    frr: None,
                    threshold: 0.8,
                },
                test: WakeTestView::default(),
            },
            speaker: SpeakerView {
                state: SpeakerState::Unavailable,
                reason: Some(reason.clone()),
                done: 0,
                needed: 0,
                recording: false,
                prompts: Vec::new(),
                last_sample: None,
                required_for_risky: true,
                last_check: None,
            },
            dictation: DictationView {
                state: DictationStateView::Unavailable,
                reason: Some(reason.clone()),
                app: None,
                typed_chars: 0,
                pending_chars: 0,
                can_undo: false,
                preview: None,
                shortcut: String::new(),
                profiles: Vec::new(),
            },
            read: ReadAloudView {
                state: ReadStateView::Unavailable,
                reason: Some(reason),
                app: None,
                index: 0,
                segments: 0,
                rate: 1.0,
                queued: 0,
                agent: "alfa".into(),
                shortcut: String::new(),
            },
            s2s: S2sView {
                available: false,
                reason: LocalizedText::new(
                    "Szybka rozmowa w chmurze wymaga klucza API dostawcy (OpenAI Realtime) — adapter w przygotowaniu.",
                    "Fast cloud conversation needs a provider API key (OpenAI Realtime) — adapter in preparation.",
                ),
            },
        }
    }
}
