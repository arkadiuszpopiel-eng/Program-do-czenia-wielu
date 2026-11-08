//! Konfiguracja automatu (`[voice.dialog]`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Przycięcie przybliżonego prefiksu (liczenie próbek).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApproxTrim {
    /// Do granicy słowa.
    Word,
    /// Do granicy zdania (fragmentu TTS) — ostrożniej.
    Sentence,
}

/// Mowa proaktywna.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProactiveMode {
    /// Dozwolona, zawsze z etykietą.
    Labeled,
    /// Wyłączona.
    Off,
}

/// Parametry automatu i barge-in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DialogConfig {
    /// Ducking TTS w dB (ujemny).
    pub duck_db: f32,
    /// Ile mowy (≠ backchannel) potwierdza twardy stop.
    pub confirm_ms: u64,
    /// Twardy stop bez transkryptu po tylu ms ciągłej mowy.
    pub max_confirm_ms: u64,
    /// Backchannel trwający dłużej niż tyle ms nie jest już backchannelem.
    pub backchannel_max_ms: u64,
    /// Mowa krótsza niż tyle ms to szum (tylko cofnięcie duckingu).
    pub min_speech_ms: u64,
    /// Słuchawki (brak echa) — agresywniejszy próg potwierdzenia.
    pub headphones: bool,
    /// Próg potwierdzenia w słuchawkach.
    pub headphones_confirm_ms: u64,
    /// Frazy backchannelu (po `fold`), np. „mhm”, „nie no dobrze”.
    pub backchannel_phrases: Vec<String>,
    /// Przycięcie przybliżonego prefiksu.
    pub approx_trim: ApproxTrim,
    /// Fillery maskujące opóźnienie (poza prefiksem, przerywalne).
    pub fillers: bool,
    /// Po ilu ms myślenia zagrać filler.
    pub filler_after_ms: u64,
    /// Mowa proaktywna.
    pub proactive: ProactiveMode,
}

impl Default for DialogConfig {
    fn default() -> Self {
        let phrases = [
            "mhm",
            "mhmm",
            "mm",
            "mmm",
            "aha",
            "acha",
            "yhm",
            "uhm",
            "tak",
            "tak tak",
            "no",
            "no tak",
            "no no",
            "no dobrze",
            "no dobra",
            "nie no",
            "nie no dobrze",
            "nie no jasne",
            "nie no spoko",
            "okej",
            "ok",
            "okay",
            "dobrze",
            "dobra",
            "jasne",
            "rozumiem",
            "super",
            "spoko",
            "w porzadku",
            "wlasnie",
            "no wlasnie",
            "pewnie",
            "yes",
            "yeah",
            "right",
            "uh huh",
            "i see",
        ];
        Self {
            duck_db: -15.0,
            confirm_ms: 200,
            max_confirm_ms: 350,
            backchannel_max_ms: 900,
            min_speech_ms: 80,
            headphones: false,
            headphones_confirm_ms: 150,
            backchannel_phrases: phrases.iter().map(|p| (*p).to_owned()).collect(),
            approx_trim: ApproxTrim::Word,
            fillers: true,
            filler_after_ms: 1200,
            proactive: ProactiveMode::Labeled,
        }
    }
}

impl DialogConfig {
    /// Próg potwierdzenia twardego stopu (słuchawki → agresywniejszy).
    pub fn effective_confirm_ms(&self) -> u64 {
        if self.headphones {
            self.headphones_confirm_ms.min(self.confirm_ms)
        } else {
            self.confirm_ms
        }
    }
}
