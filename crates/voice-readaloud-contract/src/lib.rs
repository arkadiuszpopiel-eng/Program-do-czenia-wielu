//! Kontrakt modułu `voice-readaloud` (docs/modules/voice-readaloud/SPEC.md, PLAN §6.2, §16.2 F5,
//! ACCEPTANCE F5-11): czytanie na głos zaznaczenia albo tekstu okna.
//!
//! - Źródło ([`TextSource`]): UIA `TextPattern` tylko do odczytu (zaznaczenie przez
//!   [`SelectionReader`], dokument przez `UiaPort::read_text`), zapas Ctrl+C (schowek
//!   przywracany); nigdy pola haseł ani okna Alfy/Brokera.
//! - Segmentacja na zdania z offsetami ([`segment`]), automat sterowania
//!   ([`ReadAloudMachine`]: pauza, wznów, dalej, wstecz, szybciej, wolniej, od początku, stop),
//!   synteza głosem wybranej agentki (`voice-tts`, domyślnie tylko lokalnie — `PrivacyTag::Private`).
//! - Treść jest **niezaufana** ([`UntrustedText`]): nie trafia do zdarzeń, pamięci ani modelu;
//!   do modelu tylko za zgodą ([`ShareConsent`]) i opakowana jako dane, nie polecenia.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod machine;
pub mod segment;
mod types;

use async_trait::async_trait;
use core_bus_contract::{Event, EventKind, Level};
use personas_contract::PersonaId;
use platform_contract::WindowId;

pub use machine::{ReadAloudMachine, ReadCommand};
pub use segment::{Segment, segment};
pub use types::{
    ReadAloudCfg, ReadAloudError, ReadAloudEvent, ReadControl, ReadPhase, ReadScope, ReadStatus,
    RefuseReason, ShareConsent, SourceText, UntrustedText,
};

/// Start czytania.
pub const EVENT_STARTED: &str = "voice.readaloud.started";
/// Zdanie.
pub const EVENT_SEGMENT: &str = "voice.readaloud.segment";
/// Pauza.
pub const EVENT_PAUSED: &str = "voice.readaloud.paused";
/// Wznowienie.
pub const EVENT_RESUMED: &str = "voice.readaloud.resumed";
/// Zmiana tempa.
pub const EVENT_RATE: &str = "voice.readaloud.rate";
/// Koniec tekstu.
pub const EVENT_FINISHED: &str = "voice.readaloud.finished";
/// Zatrzymanie.
pub const EVENT_STOPPED: &str = "voice.readaloud.stopped";
/// Odmowa.
pub const EVENT_REFUSED: &str = "voice.readaloud.refused";
/// Błąd syntezy / głośnika.
pub const EVENT_FAILED: &str = "voice.readaloud.failed";

/// Rodzaj zdarzenia magistrali.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

impl ReadAloudEvent {
    /// Nazwa zdarzenia.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Started { .. } => EVENT_STARTED,
            Self::Segment { .. } => EVENT_SEGMENT,
            Self::Paused => EVENT_PAUSED,
            Self::Resumed => EVENT_RESUMED,
            Self::Rate { .. } => EVENT_RATE,
            Self::Finished => EVENT_FINISHED,
            Self::Stopped => EVENT_STOPPED,
            Self::Refused { .. } => EVENT_REFUSED,
            Self::Failed { .. } => EVENT_FAILED,
        }
    }

    /// Zdarzenie magistrali (bez treści).
    pub fn to_bus_event(&self) -> Event {
        let level = match self {
            Self::Refused { .. } | Self::Failed { .. } => Level::Warn,
            Self::Segment { .. } | Self::Rate { .. } => Level::Debug,
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
    serde_json::to_value(schemars::schema_for!(ReadAloudEvent)).unwrap_or_default()
}

/// Odczyt zaznaczenia przez UIA (`TextPattern.GetSelection`) — port do dodania w
/// `platform-contract` (`UiaPort` ma dziś tylko `read_text` dokumentu); `Ok(None)` = brak
/// zaznaczenia. Implementacja musi odmawiać dla pól haseł.
pub trait SelectionReader: Send + Sync {
    /// Zaznaczony tekst w oknie (≤ `max_chars`).
    fn selection(&self, window: WindowId, max_chars: usize) -> Result<Option<String>, String>;
}

/// Źródło tekstu (okno na pierwszym planie).
pub trait TextSource: Send + Sync {
    /// Odczyt zaznaczenia albo dokumentu.
    fn read(&self, scope: ReadScope, max_chars: usize) -> Result<SourceText, ReadAloudError>;
}

/// Czytanie na głos.
#[async_trait]
pub trait ReadAloud: Send {
    /// Czyta zaznaczenie / dokument okna na pierwszym planie głosem agentki.
    async fn start(
        &mut self,
        scope: ReadScope,
        persona: PersonaId,
    ) -> Result<ReadStatus, ReadAloudError>;
    /// Krok (~20 ms): synteza, odtwarzanie, przejście do następnego zdania.
    async fn step(&mut self) -> ReadStatus;
    /// Sterowanie.
    async fn control(&mut self, control: ReadControl) -> ReadStatus;
    /// Stan.
    fn status(&self) -> ReadStatus;
    /// Treść dla modelu — tylko za zgodą, opakowana jako niezaufana.
    fn share_with_model(&self, consent: Option<&ShareConsent>) -> Result<String, ReadAloudError>;
    /// Zdarzenia od ostatniego odczytu.
    fn take_events(&mut self) -> Vec<ReadAloudEvent>;
}

#[cfg(test)]
mod tests;
