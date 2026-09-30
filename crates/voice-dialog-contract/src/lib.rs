//! Kontrakt modułu `voice-dialog` (PLAN §6.5, VOICE.md §5–§10, docs/modules/voice-dialog/SPEC.md).
//!
//! Automat rozmowy jest **czysto funkcyjny**: `step(stan, zdarzenie, teraz) → (stan, polecenia)`,
//! bez I/O. Polecenia (`Command`) wykonuje runtime potoku: ducking, stop TTS, anulowanie LLM,
//! zasób głośnika, przekazanie tury, wznowienie od punktu cięcia. Zasób „głośnik” (`SpeakerLock`)
//! jest traitem — realna implementacja przyjdzie ze `scheduler-lite`. Klasyfikacja intencji przerwania
//! (`InterruptClassifier`) i alignment słów (`WordAligner`) też są traitami.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod command;
mod config;
mod event;
mod ids;
mod state;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use command::{
    Command, DialogNotice, HeardPrefix, InterruptIntent, PrefixSource, ProactiveRejection,
    TurnSource,
};
pub use config::{ApproxTrim, DialogConfig, ProactiveMode};
pub use event::{ActivationSource, DialogEvent, MarkSource, ProactiveLabel, WordMark};
pub use ids::{TurnId, UtteranceId};
pub use state::{
    BargeIn, DialogPhase, DialogState, Interruption, PendingSpeech, SpokenChunk, UserTurn,
    Utterance,
};

use core_bus_contract::EventKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_persona_contract::PersonaId;

/// Zmiana fazy.
pub const EVENT_STATE_CHANGED: &str = "voice.dialog.state_changed";
/// Ducking.
pub const EVENT_DUCKED: &str = "voice.dialog.ducked";
/// Przerwanie (prefiks, `approximate`).
pub const EVENT_INTERRUPTED: &str = "voice.dialog.interrupted";
/// Klasyfikacja intencji.
pub const EVENT_INTENT_CLASSIFIED: &str = "voice.dialog.intent_classified";
/// Backchannel.
pub const EVENT_BACKCHANNEL: &str = "voice.dialog.backchannel";
/// Mowa proaktywna.
pub const EVENT_PROACTIVE: &str = "voice.dialog.proactive";
/// Filler.
pub const EVENT_FILLER: &str = "voice.dialog.filler";
/// Metryki (p50/p95 etapów, fałszywe przerwania).
pub const EVENT_METRICS: &str = "voice.dialog.metrics";

/// Rodzaj zdarzenia magistrali dla nazwy z tego modułu.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Wynik kroku automatu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Transition {
    /// Nowy stan.
    pub state: DialogState,
    /// Polecenia do wykonania (w kolejności).
    pub commands: Vec<Command>,
}

/// Automat rozmowy — czysta funkcja przejścia.
pub trait DialogAutomaton: Send + Sync {
    /// Jeden krok: stan + zdarzenie + czas → nowy stan + polecenia.
    fn step(&self, state: &DialogState, event: &DialogEvent, now_ms: u64) -> Transition;
}

/// Wejście klasyfikatora intencji przerwania.
#[derive(Debug, Clone, Copy)]
pub struct InterruptContext<'a> {
    /// Co użytkownik usłyszał.
    pub heard_prefix: &'a str,
    /// Czego nie usłyszał.
    pub unsaid: &'a str,
    /// Wypowiedź użytkownika.
    pub utterance: &'a str,
}

/// Wynik klasyfikacji.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct IntentResult {
    /// Intencja.
    pub intent: InterruptIntent,
    /// Pewność 0–1.
    pub confidence: f32,
}

/// Klasyfikator intencji przerwania (heurystyka PL, mały model lub LLM).
pub trait InterruptClassifier: Send + Sync {
    /// Klasyfikuje wypowiedź przerywającą.
    fn classify(&self, ctx: &InterruptContext<'_>) -> IntentResult;
}

/// Właściciel zasobu „głośnik”.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SpeakerOwner {
    /// Persona.
    pub persona: PersonaId,
    /// Wypowiedź.
    pub utterance: UtteranceId,
}

/// Głośnik zajęty.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[error("głośnik zajęty przez {} (wypowiedź {})", holder.persona, holder.utterance.0)]
pub struct SpeakerBusy {
    /// Obecny właściciel.
    pub holder: SpeakerOwner,
}

/// Zasób wyłączny „głośnik” — jedna agentka mówi naraz (docelowo `scheduler-lite`).
pub trait SpeakerLock: Send + Sync {
    /// Próbuje przejąć głośnik (ten sam właściciel ponownie → sukces).
    fn try_acquire(&self, owner: &SpeakerOwner) -> Result<(), SpeakerBusy>;
    /// Zwalnia głośnik, jeśli trzyma go `owner`; zwraca, czy zwolniono.
    fn release(&self, owner: &SpeakerOwner) -> bool;
    /// Obecny właściciel.
    fn holder(&self) -> Option<SpeakerOwner>;
}

/// Błąd alignmentu.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize, JsonSchema)]
#[error("alignment nieudany: {reason}")]
pub struct AlignError {
    /// Powód.
    pub reason: String,
}

/// Forced alignment słów na wygenerowanym audio (źródło prefiksu nr 2).
pub trait WordAligner: Send + Sync {
    /// Znaczniki słów tekstu w audio (czas względem początku audio).
    fn align(
        &self,
        text: &str,
        audio: &[f32],
        sample_rate: u32,
    ) -> Result<Vec<WordMark>, AlignError>;
}

/// Uruchamia automat na skrypcie zdarzeń `(czas, zdarzenie)`; zwraca stan końcowy i polecenia z czasem.
pub fn drive<A: DialogAutomaton + ?Sized>(
    automaton: &A,
    initial: DialogState,
    events: &[(u64, DialogEvent)],
) -> (DialogState, Vec<(u64, Command)>) {
    let mut state = initial;
    let mut out = Vec::new();
    for (now, event) in events {
        let t = automaton.step(&state, event, *now);
        state = t.state;
        out.extend(t.commands.into_iter().map(|c| (*now, c)));
    }
    (state, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_schema_and_phase_mapping() {
        for name in [
            EVENT_STATE_CHANGED,
            EVENT_DUCKED,
            EVENT_INTERRUPTED,
            EVENT_INTENT_CLASSIFIED,
            EVENT_BACKCHANNEL,
            EVENT_PROACTIVE,
            EVENT_FILLER,
            EVENT_METRICS,
        ] {
            assert!(name.starts_with("voice.dialog."));
            assert_eq!(event_kind(name).to_string(), name);
        }
        let schema = serde_json::to_string(&schemars::schema_for!(Transition)).unwrap();
        assert!(schema.contains("heard_prefix"));
        assert_eq!(
            DialogPhase::Speaking.agent_activity(),
            voice_cmd_contract::AgentActivity::Speaking
        );
        assert_eq!(
            DialogPhase::Interrupted.agent_activity(),
            voice_cmd_contract::AgentActivity::Silent
        );
        let busy = SpeakerBusy {
            holder: SpeakerOwner {
                persona: PersonaId::beta(),
                utterance: UtteranceId(3),
            },
        };
        assert!(busy.to_string().contains("beta"));
        assert!(
            DialogConfig {
                headphones: true,
                ..DialogConfig::default()
            }
            .effective_confirm_ms()
                < DialogConfig::default().effective_confirm_ms()
        );
    }

    #[test]
    fn command_serialization_is_tagged() {
        let json = serde_json::to_value(Command::DuckOutput { db: -15.0 }).unwrap();
        assert_eq!(json["type"], "duck_output");
        let ev: DialogEvent = serde_json::from_value(serde_json::json!({"event": "tick"})).unwrap();
        assert_eq!(ev, DialogEvent::Tick);
    }
}
