//! Migawka stanu potoku (UI: tryb głosowy, pigułka, wskaźnik mikrofonu).

use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_dialog_contract::{DialogPhase, HeardPrefix};
use voice_wake_contract::MicState;

/// Kto teraz mówi (pigułka).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "who", content = "persona", rename_all = "snake_case")]
pub enum Speaker {
    /// Nikt.
    #[default]
    Nobody,
    /// Użytkownik (VAD).
    User,
    /// Agentka.
    Agent(PersonaId),
}

/// Opóźnienia ostatniej tury (ms zegara potoku; `None` = etap nie wystąpił).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct TurnLatency {
    /// Tura (numer z automatu).
    pub turn: u64,
    /// Ostatnia ramka mowy wg VAD.
    pub speech_end_ms: Option<u64>,
    /// Decyzja „koniec tury”.
    pub end_of_turn_ms: Option<u64>,
    /// Final STT.
    pub stt_final_ms: Option<u64>,
    /// Pierwszy tekst odpowiedzi (TTFT).
    pub first_text_ms: Option<u64>,
    /// Pierwszy fragment TTS gotowy (TTFB).
    pub first_chunk_ms: Option<u64>,
    /// Pierwsza próbka odpowiedzi na urządzeniu wyjściowym (po opóźnieniu wyjścia).
    pub first_audio_ms: Option<u64>,
}

impl TurnLatency {
    /// Czas do pierwszego audio (od ostatniej ramki mowy) — metryka §6.4.
    pub fn time_to_first_audio_ms(&self) -> Option<u64> {
        Some(self.first_audio_ms?.saturating_sub(self.speech_end_ms?))
    }
}

/// Migawka stanu potoku.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineStatus {
    /// Czas potoku (ms).
    pub now_ms: u64,
    /// Faza automatu dialogu.
    pub phase: DialogPhase,
    /// Stan mikrofonu (jeden naraz).
    pub mic: MicState,
    /// Czy strumień mikrofonu jest otwarty.
    pub mic_open: bool,
    /// Kto mówi.
    pub speaker: Speaker,
    /// Aktywna agentka (głos i persona kolejnej odpowiedzi).
    pub persona: PersonaId,
    /// Poziom mikrofonu po DSP (dBFS, wygładzony).
    pub level_db: f32,
    /// Bieżący transkrypt częściowy użytkownika.
    pub partial: String,
    /// Ostatni usłyszany prefiks (po przerwaniu).
    pub heard_prefix: Option<HeardPrefix>,
    /// Liczba tur przekazanych do modelu.
    pub turns: u64,
    /// Liczba twardych przerwań mowy agentki.
    pub interruptions: u64,
    /// Ramki odrzucone przez regułę echa.
    pub echo_gated_frames: u64,
    /// Opóźnienia ostatniej tury.
    pub latency: TurnLatency,
}
