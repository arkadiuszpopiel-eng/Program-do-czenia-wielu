//! Konfiguracja potoku (`[voice.pipeline]`).

use personas_contract::PersonaId;
use providers_contract::PrivacyTag;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_persona_contract::ChunkerCfg;

use crate::PipelineError;

/// Reguła echa: mowa użytkownika w trakcie mowy agentki musi być wyraźnie głośniejsza niż
/// przewidywane resztkowe echo własnego TTS (po AEC) — inaczej ramka trafia do VAD jako cisza.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EchoGateCfg {
    /// Reguła włączona (wyłączenie tylko do diagnostyki/Voice Lab).
    pub enabled: bool,
    /// Okno referencji (ms): najdłuższe opóźnienie pętli + pogłos „pokoju”.
    pub tail_ms: u32,
    /// Początkowe sprzężenie głośnik → mikrofon (dB względem referencji), zanim reguła je zmierzy.
    pub initial_coupling_db: f32,
    /// Margines (dB) ponad przewidywane echo wymagany dla mowy użytkownika.
    pub margin_db: f32,
    /// Referencja cichsza niż to (dBFS) = agentka milczy (reguła nieaktywna).
    pub reference_floor_db: f32,
    /// Ramka z pewnością AEC poniżej progu jest traktowana jak echo.
    pub min_aec_confidence: f32,
}

impl Default for EchoGateCfg {
    fn default() -> Self {
        Self {
            enabled: true,
            tail_ms: 300,
            initial_coupling_db: -6.0,
            margin_db: 6.0,
            reference_floor_db: -60.0,
            min_aec_confidence: 0.5,
        }
    }
}

/// Konfiguracja potoku.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PipelineCfg {
    /// Okres kroku wątku przetwarzania (ms).
    pub tick_ms: u32,
    /// Co ile ms prosić STT o partial w trakcie barge-in (ścieżka keyword-spottera „stop/czekaj”).
    pub barge_partial_ms: u32,
    /// Ile audio sprzed decyzji VAD dołączyć na początku wypowiedzi (ms).
    pub preroll_ms: u32,
    /// Najdłuższa wypowiedź użytkownika trzymana do ponowienia STT (ms).
    pub max_utterance_ms: u32,
    /// Reguła echa.
    pub echo: EchoGateCfg,
    /// Ile audio synteza może wyprzedzać odtwarzanie (ms) — kolejka wyjścia się nie przepełnia,
    /// a przy przerwaniu jest mniej do porzucenia.
    pub speak_ahead_ms: u32,
    /// Rampa duckingu (ms, ≤ 50).
    pub duck_attack_ms: u32,
    /// Powrót głośności po duckingu (ms).
    pub unduck_release_ms: u32,
    /// Co ile ms publikować pigułkę (kto mówi, poziom).
    pub pill_every_ms: u32,
    /// Persona domyślna (Mówczyni), zanim adresowanie lub komenda ją zmieni.
    pub default_persona: PersonaId,
    /// Tag prywatności sesji (STT/TTS/LLM w chmurze tylko gdy wolno).
    pub privacy: PrivacyTag,
    /// Parametry chunkera strumienia odpowiedzi.
    pub chunker: ChunkerCfg,
    /// Tekst fillera maskującego opóźnienie (poza prefiksem, przerywalny).
    pub filler_text: String,
    /// Odpowiedź, gdy model zawiedzie przed pierwszym tekstem.
    pub failure_text: String,
    /// Bezczynność w słuchaniu do `IdleTimeout` (ms, 0 = wyłączone).
    pub idle_timeout_ms: u64,
    /// Dziennik poleceń automatu z czasem (diagnostyka, Voice Lab, testy) — liczba wpisów.
    pub trace_capacity: usize,
}

impl Default for PipelineCfg {
    fn default() -> Self {
        Self {
            tick_ms: 10,
            barge_partial_ms: 100,
            preroll_ms: 200,
            max_utterance_ms: 30_000,
            echo: EchoGateCfg::default(),
            speak_ahead_ms: 8_000,
            duck_attack_ms: 20,
            unduck_release_ms: 50,
            pill_every_ms: 100,
            default_persona: PersonaId::alfa(),
            privacy: PrivacyTag::Normal,
            chunker: ChunkerCfg::default(),
            filler_text: "Hmm, już sprawdzam.".into(),
            failure_text: "Przepraszam, nie udało mi się teraz odpowiedzieć.".into(),
            idle_timeout_ms: 0,
            trace_capacity: 0,
        }
    }
}

impl PipelineCfg {
    /// Walidacja zakresów.
    pub fn validate(&self) -> Result<(), PipelineError> {
        let bad = |what: &str| Err(PipelineError::InvalidConfig(what.to_owned()));
        if !(5..=40).contains(&self.tick_ms) {
            return bad("tick_ms poza 5–40");
        }
        if self.barge_partial_ms < self.tick_ms || self.barge_partial_ms > 1_000 {
            return bad("barge_partial_ms poza tick_ms–1000");
        }
        if self.preroll_ms > 1_000 || self.max_utterance_ms < 1_000 {
            return bad("preroll_ms ≤ 1000 i max_utterance_ms ≥ 1000");
        }
        if !(1_000..=25_000).contains(&self.speak_ahead_ms) {
            return bad("speak_ahead_ms poza 1000–25000 (kolejka wyjścia 30 s)");
        }
        if self.duck_attack_ms > 50 {
            return bad("rampa duckingu > 50 ms");
        }
        let e = &self.echo;
        if e.tail_ms > 2_000 || !(0.0..=24.0).contains(&e.margin_db) {
            return bad("echo: tail_ms ≤ 2000, margin_db 0–24");
        }
        if !(-60.0..=6.0).contains(&e.initial_coupling_db)
            || !(0.0..=1.0).contains(&e.min_aec_confidence)
        {
            return bad("echo: initial_coupling_db −60…6, min_aec_confidence 0–1");
        }
        if self.filler_text.trim().is_empty() || self.failure_text.trim().is_empty() {
            return bad("puste teksty fillera/awarii");
        }
        Ok(())
    }
}
