//! Kontrakt modułu `voice-wake` v0 (docs/modules/voice-wake/SPEC.md, PLAN §6.2, §7.3):
//! push-to-talk (wciśnięcie i puszczenie z `HotkeyPort` — hook `WH_KEYBOARD_LL`), przełącznik,
//! przycisk w UI, adresowanie po imieniu z transkryptu (`personas-contract::resolve_addressee`),
//! „nie przeszkadzać”, stan mikrofonu jako zdarzenia, mikrofon jako zasób wyłączny `scheduler-lite`
//! ([`MicArbiter`], [`lease_now`]).
//!
//! v1 (F5): słowa wywoławcze „Hej Alfa/Beta/Gama/Delta” i imiona z Kreatora — **domyślnie
//! wyłączone**, tylko lokalnie: [`WakeWordListener`] (bufor pierścieniowy ~2 s, bramka energii,
//! model KWS przez [`KeywordScorer`], detektor z histerezą i oknem odporności [`WakeWordDetector`],
//! opcjonalna bramka właściciela [`OwnerCheck`]) → [`WakeInput::WakeWord`] → słuchanie
//! z adresatką; metryki FAR/FRR w [`eval`]. Tryb „zawsze słucham” jest poza v1 (odrzucany).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod detector;
pub mod eval;
mod listener;
mod machine;
mod mic;
mod words;

use core_bus_contract::{Event, EventKind, Level};
use personas_contract::PersonaId;
use platform_contract::{Hotkey, HotkeyId, Key, Modifiers};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use detector::{KwsScores, WakeWordDetector, WakeWordHit};
pub use listener::{
    KWS_RATE, KeywordScorer, ListenerStats, OwnerCheck, WakeWordListener, WakeWordTrigger,
};
pub use machine::WakeMachine;
pub use mic::{MicArbiter, lease_now};
pub use words::{
    KwsParams, MAX_PHRASE_CHARS, MAX_PHRASES, MIN_PHRASE_SYLLABLES, normalize_phrase, syllables,
};

/// Początek słuchania.
pub const EVENT_LISTEN_START: &str = "voice.wake.listen_start";
/// Koniec słuchania.
pub const EVENT_LISTEN_STOP: &str = "voice.wake.listen_stop";
/// Adresatka wypowiedzi (po imieniu albo Dyrygentka).
pub const EVENT_ADDRESSED: &str = "voice.wake.addressed";
/// Okno administratora na pierwszym planie — hook PTT nie działa (UI mówi to wprost).
pub const EVENT_BLOCKED_ELEVATED: &str = "voice.wake.blocked_elevated_foreground";
/// Tryb „nie przeszkadzać”.
pub const EVENT_DND: &str = "voice.wake.dnd";
/// Stan mikrofonu (UI: przycisk, pigułka, wskaźnik prywatności).
pub const EVENT_MIC_STATE: &str = "voice.wake.mic_state";
/// Podejrzenie fałszywego wybudzenia (v1).
pub const EVENT_FALSE_ALARM: &str = "voice.wake.false_alarm_suspected";
/// Wykrycie słowa wywoławczego zignorowane (v1: wyłączone, DND, wyciszenie, trwa słuchanie).
pub const EVENT_WORD_IGNORED: &str = "voice.wake.word_ignored";

/// Rodzaj zdarzenia magistrali.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Stan mikrofonu (wyłączony / słucha / słyszy / przetwarza / wyciszony).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MicState {
    /// Wyłączony.
    Off,
    /// Słucha (PTT/przełącznik aktywny, cisza).
    Listening,
    /// Słyszy mowę (VAD).
    Hearing,
    /// Przetwarza wypowiedź (STT/LLM).
    Processing,
    /// Wyciszony przez użytkownika.
    Muted,
}

/// Źródło aktywacji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WakeSource {
    /// Przytrzymanie klawisza globalnego.
    Ptt,
    /// Przełącznik globalny.
    Toggle,
    /// Słowo wywoławcze (v1).
    WakeWord,
    /// Imię w transkrypcie.
    Name,
    /// Przycisk / Spacja w oknie aplikacji.
    Ui,
}

/// Wejście automatu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WakeInput {
    /// Zdarzenie skrótu z `HotkeyPort` (wciśnięcie / puszczenie).
    Key {
        /// Skrót.
        id: HotkeyId,
        /// Wciśnięty (`true`) albo puszczony.
        pressed: bool,
    },
    /// Przycisk mikrofonu / Spacja w UI (przytrzymanie).
    UiPtt {
        /// Wciśnięty.
        pressed: bool,
    },
    /// Kliknięcie przycisku mikrofonu (przełącznik).
    UiToggle,
    /// VAD: mowa / cisza.
    Vad {
        /// Mowa.
        speech: bool,
    },
    /// STT/LLM przetwarza wypowiedź.
    Processing {
        /// Zajęty.
        busy: bool,
    },
    /// Transkrypt (final) — adresowanie po imieniu.
    Transcript {
        /// Tekst.
        text: String,
    },
    /// Wyciszenie mikrofonu.
    SetMuted {
        /// Wyciszony.
        muted: bool,
    },
    /// „Nie przeszkadzać”.
    SetDnd {
        /// Włączony.
        on: bool,
    },
    /// Okno podniesione (administrator) na pierwszym planie.
    ElevatedForeground {
        /// Podniesione.
        elevated: bool,
    },
    /// v1: wykryto słowo wywoławcze ([`WakeWordListener`]) — otwiera słuchanie z adresatką.
    WakeWord {
        /// Adresatka z frazy.
        persona: PersonaId,
        /// Fraza („Hej Delta”).
        phrase: String,
        /// Czas wykrycia (ms, zegar potoku).
        at_ms: u64,
    },
    /// v1: upływ czasu (ms, zegar potoku) — zamyka sesję słowa wywoławczego po ciszy.
    Tick {
        /// Teraz (ms).
        now_ms: u64,
    },
}

/// Dlaczego wykrycie słowa wywoławczego nie otworzyło słuchania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WakeIgnoreReason {
    /// Słowa wywoławcze wyłączone (domyślnie).
    Disabled,
    /// Mikrofon wyciszony.
    Muted,
    /// „Nie przeszkadzać”.
    DoNotDisturb,
    /// Słuchanie już trwa.
    AlreadyListening,
}

/// Zdarzenia modułu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WakeEvent {
    /// `voice.wake.listen_start`.
    ListenStart {
        /// Adresatka znana z góry (v1: słowo wywoławcze „Hej Delta”).
        addressed: Option<PersonaId>,
        /// Źródło.
        source: WakeSource,
    },
    /// `voice.wake.listen_stop`.
    ListenStop {
        /// Źródło, które otworzyło słuchanie.
        source: WakeSource,
    },
    /// `voice.wake.addressed`.
    Addressed {
        /// Adresatka.
        persona: PersonaId,
        /// Po imieniu (`false` = Dyrygentka domyślnie).
        by_name: bool,
    },
    /// `voice.wake.blocked_elevated_foreground`.
    BlockedElevatedForeground,
    /// `voice.wake.dnd`.
    Dnd {
        /// Włączony.
        on: bool,
    },
    /// `voice.wake.mic_state`.
    MicState {
        /// Stan.
        state: MicState,
    },
    /// `voice.wake.false_alarm_suspected` — po wybudzeniu nie padło ani słowo (licznik FAR w UI).
    FalseAlarmSuspected {
        /// Adresatka z frazy.
        persona: PersonaId,
        /// Fraza.
        phrase: String,
    },
    /// `voice.wake.word_ignored`.
    WakeWordIgnored {
        /// Adresatka z frazy.
        persona: PersonaId,
        /// Powód.
        reason: WakeIgnoreReason,
    },
}

impl WakeEvent {
    /// Nazwa zdarzenia.
    pub fn name(&self) -> &'static str {
        match self {
            WakeEvent::ListenStart { .. } => EVENT_LISTEN_START,
            WakeEvent::ListenStop { .. } => EVENT_LISTEN_STOP,
            WakeEvent::Addressed { .. } => EVENT_ADDRESSED,
            WakeEvent::BlockedElevatedForeground => EVENT_BLOCKED_ELEVATED,
            WakeEvent::Dnd { .. } => EVENT_DND,
            WakeEvent::MicState { .. } => EVENT_MIC_STATE,
            WakeEvent::FalseAlarmSuspected { .. } => EVENT_FALSE_ALARM,
            WakeEvent::WakeWordIgnored { .. } => EVENT_WORD_IGNORED,
        }
    }

    /// Zdarzenie magistrali.
    pub fn to_bus_event(&self) -> Event {
        let level = match self {
            WakeEvent::BlockedElevatedForeground => Level::Warn,
            _ => Level::Info,
        };
        Event::new(
            event_kind(self.name()),
            level,
            serde_json::to_value(self).unwrap_or_default(),
        )
    }
}

/// JSON Schema zdarzeń.
pub fn event_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(WakeEvent)).unwrap_or_default()
}

/// Słowa wywoławcze (v1, F5): frazy → persony, próg detektora, bramka właściciela. Walidacja:
/// [`WakeWordCfg::validate`]; włączenie rekomendowane po FAR ≤ 1/dzień i FRR ≤ 5% (F5-05/06).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WakeWordCfg {
    /// Frazy („Hej Alfa”…) przypisane do person.
    pub phrases: Vec<(String, PersonaId)>,
    /// Próg detektora KWS.
    pub threshold: f32,
    /// „Zawsze słucham” — wymaga bramki właściciela (`voice-speaker`).
    pub always_on: bool,
    /// Bramka właściciela.
    pub owner_gate: bool,
}

/// Konfiguracja (`[voice.wake]`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WakeCfg {
    /// Globalny PTT (hook — zgłasza puszczenie). Spacja bez modyfikatora działa tylko w UI.
    pub ptt_key: Option<Hotkey>,
    /// Globalny przełącznik.
    pub toggle_key: Option<Hotkey>,
    /// Adresowanie po imieniu z transkryptu.
    pub name_addressing: bool,
    /// v1 (F5): słowa wywoławcze — `None` = wyłączone (domyślnie).
    pub wake_words: Option<WakeWordCfg>,
}

impl Default for WakeCfg {
    /// PTT `Ctrl+Shift+Space`, przełącznik `Ctrl+Shift+M` (SPEC; bez kolizji z AltGr i kill-switchem).
    fn default() -> Self {
        let cs = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::default()
        };
        Self {
            ptt_key: Some(Hotkey::new(cs, Key::Space)),
            toggle_key: Some(Hotkey::new(cs, Key::Letter('M'))),
            name_addressing: true,
            wake_words: None,
        }
    }
}

impl WakeCfg {
    /// Walidacja: reguła AltGr/kill-switch dla skrótów, różne klawisze, słowa wywoławcze
    /// ([`WakeWordCfg::validate`]: frazy ≥ 3 sylaby, próg w (0, 1), bez „zawsze słucham”).
    pub fn validate(&self) -> Result<(), WakeError> {
        for k in [self.ptt_key, self.toggle_key].into_iter().flatten() {
            k.validate().map_err(|e| WakeError::Hotkey(e.to_string()))?;
        }
        if self.ptt_key.is_some() && self.ptt_key == self.toggle_key {
            return Err(WakeError::InvalidConfig(
                "PTT i przełącznik na tym samym skrócie".into(),
            ));
        }
        if let Some(w) = &self.wake_words {
            w.validate()?;
        }
        Ok(())
    }
}

/// Błędy modułu.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum WakeError {
    /// Niepoprawna konfiguracja.
    #[error("niepoprawna konfiguracja wybudzania: {0}")]
    InvalidConfig(String),
    /// Skrót odrzucony (AltGr, kill-switch, zajęty).
    #[error("skrót: {0}")]
    Hotkey(String),
    /// Funkcja niedostępna (brak fraz, tryb poza v1).
    #[error("niedostępne: {0}")]
    NotAvailable(String),
    /// Model KWS (ładowanie, hash, inferencja).
    #[error("model słów wywoławczych: {0}")]
    Model(String),
}

/// Aktywacja słuchania i adresowanie.
pub trait Wake: Send {
    /// Ustawia konfigurację (rejestruje skróty).
    fn configure(&mut self, cfg: WakeCfg) -> Result<(), WakeError>;
    /// Odbiera zdarzenia skrótów (PTT/przełącznik) i stan okna podniesionego.
    fn pump(&mut self) -> Vec<WakeEvent>;
    /// Wejście z potoku (UI, VAD, STT, wyciszenie, DND).
    fn handle(&mut self, input: WakeInput) -> Vec<WakeEvent>;
    /// Adresatka wypowiedzi („Delta, …” / „Hej Gama” → imię; bez imienia — Dyrygentka).
    fn addressed(&self, text: &str) -> Option<PersonaId>;
    /// Stan mikrofonu.
    fn mic_state(&self) -> MicState;
    /// Czy trwa słuchanie.
    fn is_listening(&self) -> bool;
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod v1_tests;
