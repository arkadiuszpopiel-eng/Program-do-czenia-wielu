//! Port `ReplySource`: generowanie odpowiedzi mówionej (strumień tekstu) i zapis wyniku tury do
//! historii append-only. Implementacja z `ModelProvider` + historią jest w `voice-pipeline-impl`
//! (`ProviderReply`); `app-core` podepnie prawdziwy czat sesji.

use std::pin::Pin;

use futures_core::Stream;
use personas_contract::PersonaId;
use providers_contract::CancellationToken;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_dialog_contract::{InterruptIntent, TurnSource};
use voice_speaker_contract::{SpeakerCheck, voice_origin};

/// Żądanie odpowiedzi na turę użytkownika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReplyRequest {
    /// Tura (numer z automatu dialogu).
    pub turn: u64,
    /// Agentka odpowiadająca (głos i persona).
    pub persona: PersonaId,
    /// Tekst użytkownika.
    pub text: String,
    /// Źródło (głos / composer).
    pub source: TurnSource,
    /// Intencja, gdy tura przerwała mowę agentki (korekta, uzupełnienie…).
    pub intent: Option<InterruptIntent>,
    /// Pochodzenie tury głosowej (pewność STT, weryfikacja mówcy) — źródło polecenia dla
    /// klasyfikatora ryzyka ([`VoiceProvenance::command_origin`]); `None` dla tekstu.
    #[serde(default)]
    pub voice: Option<VoiceProvenance>,
}

/// Pochodzenie tury głosowej (F5): sygnał do `risk-classifier` przez istniejące pola
/// `CommandOrigin::UserVoice { confidence, speaker_verified }`. Bez weryfikacji (`NotChecked`,
/// `Pending`) — `speaker_verified = false`, więc akcje ryzykowne wymagają potwierdzenia nie-głosem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VoiceProvenance {
    /// Pewność finalu STT (‰).
    pub stt_confidence_permille: u16,
    /// Weryfikacja mówcy (wynik może dojść później: [`ReplySource::speaker_checked`]).
    pub speaker: SpeakerCheck,
}

impl VoiceProvenance {
    /// Źródło polecenia dla klasyfikatora ryzyka / Brokera.
    pub fn command_origin(&self) -> risk_classifier_contract::CommandOrigin {
        voice_origin(
            f32::from(self.stt_confidence_permille) / 1000.0,
            &self.speaker,
        )
    }
}

/// Element strumienia odpowiedzi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "chunk", content = "text", rename_all = "snake_case")]
pub enum ReplyChunk {
    /// Kolejny fragment tekstu (markdown — kanał mówiony wydziela `voice-persona`).
    Text(String),
    /// Koniec odpowiedzi.
    Done,
    /// Błąd (PL) — strumień się kończy.
    Failed(String),
}

/// Wynik tury asystentki dla historii (append-only: tura trafia do historii raz, z wynikiem).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ReplyOutcome {
    /// Wysłuchana do końca.
    Completed,
    /// Przerwana — użytkownik usłyszał tylko prefiks (w tekście oryginału odpowiedzi).
    Interrupted {
        /// Usłyszany prefiks.
        heard: String,
        /// Granica przybliżona (liczenie próbek).
        approximate: bool,
    },
}

/// Strumień odpowiedzi (kończy się po `Done` / `Failed`; upuszczenie przerywa generowanie).
pub type ReplyStream = Pin<Box<dyn Stream<Item = ReplyChunk> + Send + 'static>>;

/// Źródło odpowiedzi mówionych.
pub trait ReplySource: Send + Sync {
    /// Zaczyna odpowiedź na turę; anulowanie `cancel` (barge-in) kończy strumień ≤ 100 ms.
    /// Wcześniejsze tury muszą być już zamknięte przez [`ReplySource::finish`].
    fn start(&self, request: ReplyRequest, cancel: CancellationToken) -> ReplyStream;
    /// Zamyka turę asystentki z wynikiem (pełna treść + ewentualny usłyszany prefiks).
    fn finish(&self, turn: u64, outcome: ReplyOutcome);
    /// Wynik weryfikacji mówcy dla tury głosowej, która wystartowała z `SpeakerCheck::Pending`
    /// (weryfikacja biegnie równolegle z odpowiedzią). Domyślnie ignorowane — do czasu wyniku
    /// tura jest niezweryfikowana.
    fn speaker_checked(&self, turn: u64, check: SpeakerCheck) {
        let _ = (turn, check);
    }
}
