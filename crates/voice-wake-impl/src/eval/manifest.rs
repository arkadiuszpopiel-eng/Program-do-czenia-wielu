//! Manifest zestawu słów wywoławczych F5 (`evals/F5/voice/README.md`, NDJSON — jedna pozycja na
//! linię): pozytywy właściciela („Hej Alfa…”, typ 10 protokołu nagrań) i nagrania tła (TV,
//! podcasty, rozmowy) do FAR. Audio: WAV 16 kHz mono PCM16 (ścieżka względna) albo przepis
//! próbki syntetycznej (`synth`, CI).

use std::collections::BTreeSet;
use std::path::{Component, Path};

use personas_contract::PersonaId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Rodzaj pozycji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WakeItemKind {
    /// Fraza wywoławcza właściciela (FRR).
    WakePositive,
    /// Tło bez frazy (FAR).
    WakeBackground,
}

/// Podział.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    /// Strojenie (progi, histereza).
    Dev,
    /// Ocena (zamrażany hashem).
    Test,
}

/// Fragment pliku (ms) — okno, w którym ma paść wykrycie pozytywu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Segment {
    /// Początek frazy.
    pub start_ms: u64,
    /// Koniec frazy.
    pub end_ms: u64,
}

/// Warunki nagrania.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Conditions {
    /// `desktop` / `laptop` / `synthetic`.
    pub machine: String,
    /// `quiet` / `noise` / `speakers` / `tv` / `podcast` / `conversation`.
    pub environment: String,
    /// `usb` / `builtin` / `headset` / `virtual`.
    pub mic: String,
    /// Odległość od mikrofonu (m).
    #[serde(default)]
    pub distance_m: Option<f32>,
}

/// Przepis próbki syntetycznej (CI): cisza/szum, potem ton (atrapa „frazy”) albo mowa syntetyczna.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SynthRecipe {
    /// `tone` (fraza atrapy: ton o `tone_hz`) albo `speech` (mowa syntetyczna, tło).
    pub kind: String,
    /// Długość części głównej (s).
    pub secs: f32,
    /// Cichy szum przed (s).
    #[serde(default)]
    pub lead_secs: f32,
    /// Częstotliwość tonu (Hz).
    #[serde(default)]
    pub tone_hz: f32,
    /// Ziarno.
    #[serde(default)]
    pub seed: u64,
}

/// Pozycja manifestu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WakeItem {
    /// Unikalny identyfikator `[a-z0-9._-]`.
    pub id: String,
    /// Plik WAV 16 kHz mono (względnie wobec `--audio-root`, bez `..`).
    #[serde(default)]
    pub audio: Option<String>,
    /// Rodzaj.
    pub kind: WakeItemKind,
    /// Podział.
    pub split: Split,
    /// Adresatka pozytywu (`alfa`, `delta`, własne imię z Kreatora).
    #[serde(default)]
    pub persona: Option<String>,
    /// Wypowiedziana fraza.
    #[serde(default)]
    pub phrase: Option<String>,
    /// Gdzie w pliku jest fraza.
    #[serde(default)]
    pub segment: Option<Segment>,
    /// Warunki.
    pub conditions: Conditions,
    /// Przepis syntetyczny (zamiast `audio`).
    #[serde(default)]
    pub synth: Option<SynthRecipe>,
    /// Uwagi.
    #[serde(default)]
    pub note: Option<String>,
}

impl WakeItem {
    /// Adresatka pozytywu.
    pub fn persona_id(&self) -> Option<PersonaId> {
        self.persona.as_deref().and_then(PersonaId::parse)
    }
}

/// Parsuje NDJSON (puste linie i `#…` pomijane); błąd z numerem linii.
pub fn parse_manifest(text: &str) -> Result<Vec<WakeItem>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|(i, l)| serde_json::from_str(l).map_err(|e| format!("linia {}: {e}", i + 1)))
        .collect()
}

fn safe_audio(path: &str) -> bool {
    !path.contains(['\\', ':'])
        && path.ends_with(".wav")
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

/// Walidacja spójności (identyfikatory, ścieżki, pola wymagane per rodzaj). Zwraca listę problemów.
pub fn validate_manifest(items: &[WakeItem]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut ids = BTreeSet::new();
    let mut files: std::collections::BTreeMap<&str, Split> = std::collections::BTreeMap::new();
    for it in items {
        let p = |m: &str| format!("{}: {m}", it.id);
        let id_ok = !it.id.is_empty()
            && it
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c));
        if !id_ok {
            problems.push(p("identyfikator musi być [a-z0-9._-]"));
        }
        if !ids.insert(it.id.as_str()) {
            problems.push(p("identyfikator powtórzony"));
        }
        match (&it.audio, &it.synth) {
            (Some(a), None) if !safe_audio(a) => {
                problems.push(p("audio: ścieżka względna .wav bez `..`, `\\`, `:`"))
            }
            (Some(a), None) => {
                if files
                    .insert(a.as_str(), it.split)
                    .is_some_and(|s| s != it.split)
                {
                    problems.push(p("ten sam plik w dev i test"));
                }
            }
            (None, Some(s)) => {
                if !["tone", "speech"].contains(&s.kind.as_str())
                    || !(0.1..=3600.0).contains(&s.secs)
                {
                    problems.push(p("synth: kind tone|speech, secs 0,1–3600"));
                }
            }
            _ => problems.push(p("dokładnie jedno z `audio` i `synth`")),
        }
        if it.kind == WakeItemKind::WakePositive {
            if it.persona_id().is_none() {
                problems.push(p("pozytyw wymaga `persona` (identyfikator persony)"));
            }
            if it.segment.is_some_and(|s| s.end_ms <= s.start_ms) {
                problems.push(p("segment: end_ms > start_ms"));
            }
        }
    }
    problems
}
