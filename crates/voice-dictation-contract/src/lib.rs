//! Kontrakt modułu `voice-dictation` (docs/modules/voice-dictation/SPEC.md, PLAN §6.2, §7.3,
//! §16.2 F5, ACCEPTANCE F5-10): dyktowanie do dowolnej aplikacji.
//!
//! - STT (final) → [`normalize`] (komendy interpunkcji PL, liczby tylko jednoznaczne) → wpisanie
//!   tekstu przez `InputPort` (`SendInput` Unicode) **tylko do okna, które było na pierwszym planie
//!   w chwili startu** ([`DictationTarget`]); zmiana okna → pauza, powrót → wznowienie.
//! - Nigdy do okien Alfy/Brokera (`TargetGuard`), okien administratora (UIPI) ani pól haseł
//!   (UIA `IsPassword` → odmowa); w terminalach Enter z „nowa linia” jest blokowany.
//! - „cofnij to” — usuwa ostatnią frazę (Backspace), „koniec dyktowania” — kończy; tryb
//!   push-to-talk i przełącznik ([`DictationMode`]).
//! - Zdarzenia bez treści (liczby znaków, nazwa aplikacji) — dyktowany tekst nie trafia na
//!   magistralę, do logów ani do pamięci.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod machine;
pub mod normalize;
pub mod numbers;
mod types;

use core_bus_contract::{Event, EventKind, Level};
use personas_contract::fold;

pub use machine::{DictationAction, DictationMachine};
pub use normalize::{TextContext, normalize};
pub use types::{
    DictationCfg, DictationError, DictationEvent, DictationMode, DictationPhase, DictationStatus,
    DictationTarget, PauseReason, RefuseReason, StopReason, TERMINAL_IMAGES, is_terminal_image,
};

/// Start sesji.
pub const EVENT_STARTED: &str = "voice.dictation.started";
/// Wpisano znaki.
pub const EVENT_TYPED: &str = "voice.dictation.typed";
/// Pauza (zmiana okna, użytkownik pisze).
pub const EVENT_PAUSED: &str = "voice.dictation.paused";
/// Wznowienie.
pub const EVENT_RESUMED: &str = "voice.dictation.resumed";
/// Koniec sesji.
pub const EVENT_STOPPED: &str = "voice.dictation.stopped";
/// Odmowa (okno chronione, administratora, pole hasła).
pub const EVENT_REFUSED: &str = "voice.dictation.refused";
/// „cofnij to”.
pub const EVENT_UNDONE: &str = "voice.dictation.undone";
/// „cofnij to” niemożliwe (okno czasu, zmiana okna).
pub const EVENT_UNDO_UNAVAILABLE: &str = "voice.dictation.undo_unavailable";
/// Enter zablokowany w terminalu.
pub const EVENT_NEWLINE_BLOCKED: &str = "voice.dictation.newline_blocked";

/// Rodzaj zdarzenia magistrali.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

impl DictationEvent {
    /// Nazwa zdarzenia.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Started { .. } => EVENT_STARTED,
            Self::Typed { .. } => EVENT_TYPED,
            Self::Paused { .. } => EVENT_PAUSED,
            Self::Resumed => EVENT_RESUMED,
            Self::Stopped { .. } => EVENT_STOPPED,
            Self::Refused { .. } => EVENT_REFUSED,
            Self::Undone { .. } => EVENT_UNDONE,
            Self::UndoUnavailable => EVENT_UNDO_UNAVAILABLE,
            Self::NewlineBlocked => EVENT_NEWLINE_BLOCKED,
        }
    }

    /// Zdarzenie magistrali (bez treści dyktowania).
    pub fn to_bus_event(&self) -> Event {
        let level = match self {
            Self::Refused { .. } | Self::NewlineBlocked => Level::Warn,
            Self::Typed { .. } => Level::Debug,
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
    serde_json::to_value(schemars::schema_for!(DictationEvent)).unwrap_or_default()
}

/// Komenda sterująca (cała wypowiedź).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCommand {
    /// „cofnij to”, „usuń to”, „skasuj to”.
    Undo,
    /// „koniec dyktowania”, „zakończ dyktowanie”, „stop dyktowanie”.
    Stop,
}

/// Rozpoznaje komendę sterującą — tylko gdy cała wypowiedź jest komendą (dyktowany tekst
/// zawierający te słowa jest wpisywany).
pub fn control_command(text: &str) -> Option<ControlCommand> {
    let norm: String = fold(text)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let norm = norm.split_whitespace().collect::<Vec<_>>().join(" ");
    match norm.as_str() {
        "cofnij to" | "cofnij" | "usun to" | "skasuj to" | "cofnij ostatnie" => {
            Some(ControlCommand::Undo)
        }
        "koniec dyktowania" | "zakoncz dyktowanie" | "stop dyktowanie" | "przestan dyktowac" => {
            Some(ControlCommand::Stop)
        }
        _ => None,
    }
}

/// Dyktowanie (usługa na portach platformy w `-impl`, wirtualne pole w `-fake`).
pub trait Dictation: Send {
    /// Start: cel = okno na pierwszym planie teraz (odmowa: okno chronione, administratora, pole
    /// hasła, brak okna).
    fn start(
        &mut self,
        mode: DictationMode,
        now_ms: u64,
    ) -> Result<DictationStatus, DictationError>;
    /// Koniec (niewpisany tekst przepada).
    fn stop(&mut self) -> DictationStatus;
    /// Final STT (komenda sterująca albo fraza do wpisania).
    fn on_final(&mut self, text: &str, now_ms: u64) -> Result<DictationStatus, DictationError>;
    /// Krok: obserwacja okna na pierwszym planie (pauza/wznowienie), ponowienie zaległego tekstu.
    fn tick(&mut self, now_ms: u64) -> DictationStatus;
    /// „cofnij to” z UI.
    fn undo_last(&mut self, now_ms: u64) -> Result<DictationStatus, DictationError>;
    /// Stan.
    fn status(&self) -> DictationStatus;
    /// Zdarzenia od ostatniego odczytu.
    fn take_events(&mut self) -> Vec<DictationEvent>;
}

#[cfg(test)]
mod tests;
