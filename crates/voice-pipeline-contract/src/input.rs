//! Wejścia potoku z UI / powłoki / innych modułów (skróty globalne przychodzą przez `voice-wake`).

use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Wejście potoku.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "input", rename_all = "snake_case")]
pub enum PipelineInput {
    /// Przycisk mikrofonu / Spacja w oknie aplikacji (przytrzymanie).
    Ptt {
        /// Wciśnięty.
        pressed: bool,
    },
    /// Przełącznik mikrofonu (tryb rozmowy).
    Toggle,
    /// Użytkownik napisał w composerze (przerywa mowę i myślenie).
    Typed {
        /// Tekst.
        text: String,
    },
    /// `Esc` / „stop mowy” (nie zabija pracy w tle).
    StopSpeech,
    /// Wyjście z trybu głosowego (zatrzymuje mowę, generowanie i słuchanie).
    Deactivate,
    /// Wyciszenie mikrofonu.
    SetMuted {
        /// Wyciszony.
        muted: bool,
    },
    /// „Nie przeszkadzać”.
    SetDoNotDisturb {
        /// Włączony.
        on: bool,
    },
    /// Zmiana mówiącej agentki z UI (obsada w locie, bez restartu).
    SwitchPersona {
        /// Persona.
        persona: PersonaId,
    },
    /// Mowa proaktywna (narracja, przypomnienie) — zawsze z etykietą.
    Proactive {
        /// Kto mówi.
        persona: PersonaId,
        /// Tekst.
        text: String,
        /// Dlaczego (etykieta w UI).
        reason: String,
    },
}
