//! Stan automatu (czysta wartość — automat nie ma I/O).

use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_cmd_contract::AgentActivity;

use crate::{HeardPrefix, InterruptIntent, MarkSource, ProactiveLabel, UtteranceId, WordMark};

/// Faza rozmowy (PLAN §6.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DialogPhase {
    /// Nie słuchamy.
    #[default]
    Idle,
    /// Słuchamy, cisza.
    Listening,
    /// Użytkownik mówi.
    UserSpeaking,
    /// Generowanie odpowiedzi (lub oczekiwanie na głośnik).
    Thinking,
    /// Agentka mówi.
    Speaking,
    /// Twardy stop po przerwaniu; użytkownik kończy wypowiedź.
    Interrupted,
}

impl DialogPhase {
    /// Aktywność agentki dla `voice-cmd` (reguła „nie” tylko w `Speaking`).
    pub fn agent_activity(self) -> AgentActivity {
        match self {
            Self::Speaking => AgentActivity::Speaking,
            Self::Thinking => AgentActivity::Thinking,
            _ => AgentActivity::Silent,
        }
    }
}

/// Fragment wypowiedzi przekazany do TTS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpokenChunk {
    /// Tekst.
    pub text: String,
    /// Długość audio.
    pub audio_ms: u64,
    /// Początek fragmentu w tekście wypowiedzi (znaki).
    pub char_start: usize,
    /// Początek fragmentu w audio wypowiedzi.
    pub ms_start: u64,
    /// Znaczniki słów (jeśli są).
    pub marks: Option<Vec<WordMark>>,
    /// Źródło znaczników.
    pub mark_source: Option<MarkSource>,
}

/// Wypowiedź agentki (bieżąca albo ostatnia).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Utterance {
    /// Identyfikator.
    pub id: UtteranceId,
    /// Persona.
    pub persona: PersonaId,
    /// Fragmenty w kolejności.
    pub chunks: Vec<SpokenChunk>,
    /// Pozycja odtwarzania na urządzeniu (po korekcie opóźnienia).
    pub played_ms: u64,
    /// Etykieta mowy proaktywnej.
    pub proactive: Option<ProactiveLabel>,
}

impl Utterance {
    /// Pełny tekst wypowiedzi (fragmenty rozdzielone spacją).
    pub fn full_text(&self) -> String {
        self.chunks
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Kandydat na przerwanie (mowa użytkownika podczas `Speaking`/`Thinking`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BargeIn {
    /// Początek mowy.
    pub started_at_ms: u64,
    /// Transkrypt częściowy.
    pub partial: String,
    /// Czy wyciszono TTS.
    pub ducked: bool,
}

/// Wynik ostatniego przerwania.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Interruption {
    /// Przerwana wypowiedź.
    pub utterance: UtteranceId,
    /// Persona.
    pub persona: PersonaId,
    /// Co użytkownik usłyszał.
    pub heard: HeardPrefix,
    /// Czego nie usłyszał (reszta znanego tekstu).
    pub unsaid: String,
    /// Czas przerwania.
    pub at_ms: u64,
    /// Intencja (po klasyfikacji).
    pub intent: Option<InterruptIntent>,
}

/// Wypowiedź czekająca na głośnik.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PendingSpeech {
    /// Identyfikator nadany przez automat.
    pub utterance: UtteranceId,
    /// Persona.
    pub persona: PersonaId,
    /// Tekst mowy proaktywnej lub wznowienia (gdy znany z góry).
    pub text: Option<String>,
    /// Etykieta mowy proaktywnej.
    pub proactive: Option<ProactiveLabel>,
}

/// Bieżąca tura użytkownika.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UserTurn {
    /// Transkrypt.
    pub text: String,
    /// Początek mowy.
    pub started_at_ms: Option<u64>,
}

/// Stan automatu.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DialogState {
    /// Faza.
    pub phase: DialogPhase,
    /// „Nie przeszkadzać”.
    pub do_not_disturb: bool,
    /// VAD: użytkownik mówi teraz.
    pub vad_active: bool,
    /// Wyjście TTS wyciszone (ducking).
    pub output_ducked: bool,
    /// Automat trzyma zasób „głośnik” dla tej wypowiedzi.
    pub speaker_held: Option<UtteranceId>,
    /// Następny identyfikator wypowiedzi.
    pub next_utterance: u64,
    /// Liczba tur przekazanych do LLM.
    pub turns: u64,
    /// Tura użytkownika.
    pub user: UserTurn,
    /// Bieżąca / ostatnia wypowiedź agentki.
    pub utterance: Option<Utterance>,
    /// Wypowiedź czekająca na głośnik.
    pub pending: Option<PendingSpeech>,
    /// Kandydat na przerwanie.
    pub barge_in: Option<BargeIn>,
    /// Ostatnie przerwanie (do wznowienia).
    pub interruption: Option<Interruption>,
    /// Początek myślenia (fillery).
    pub thinking_since_ms: Option<u64>,
    /// Filler zagrany w tej turze.
    pub filler_played: bool,
}

impl DialogState {
    /// Czy agentka mówi wypowiedź `id` (zdarzenia dla innych wypowiedzi są nieaktualne).
    pub fn is_speaking(&self, id: UtteranceId) -> bool {
        self.phase == DialogPhase::Speaking && self.utterance.as_ref().is_some_and(|u| u.id == id)
    }
}
