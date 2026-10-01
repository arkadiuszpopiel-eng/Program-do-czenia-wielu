//! Manifest zestawu F2 (NDJSON, jedna pozycja na linię): plik audio, transkrypt referencyjny,
//! rodzaj nagrania (typy 1–12 protokołu nagrań), warunki, podział dev/test, etykiety.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_cmd_contract::CommandKind;
use voice_dialog_contract::InterruptIntent;

/// Rodzaj nagrania (typy z `evals/spikes/e-voice-lab/protokol-nagran.md`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// Typy 1–4: swobodna mowa (WER).
    FreeSpeech,
    /// Typ 5: mieszane PL/EN (WER).
    MixedPlEn,
    /// Typ 6: nazwy własne (WER).
    Names,
    /// Typ 7: komendy (`stop`, `anuluj`… — recall, reakcja).
    Command,
    /// Typ 8: przerwania w trakcie TTS (klasy intencji, prefiks).
    Interruption,
    /// Typ 9: backchannel w trakcie TTS (precision).
    Backchannel,
    /// Typ 10: słowa wywoławcze (F5).
    Wake,
    /// Typ 11: enrollment mówcy (F5).
    Enroll,
    /// Typ 12: dyktowanie z interpunkcją mówioną (F5, WER).
    Dictation,
}

impl ItemKind {
    /// Czy pozycja liczy się do WER.
    pub fn is_wer(self) -> bool {
        matches!(
            self,
            Self::FreeSpeech | Self::MixedPlEn | Self::Names | Self::Dictation
        )
    }
}

/// Podział zestawu (test zamrożony hashem przed implementacją F2).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Split {
    /// Strojenie.
    Dev,
    /// Ocena (zamrożony).
    Test,
}

/// Maszyna nagrania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Machine {
    /// Desktop (Standard-AMD).
    Desktop,
    /// Laptop (Laptop-CUDA).
    Laptop,
    /// Próbka syntetyczna (CI).
    Synthetic,
}

/// Otoczenie nagrania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    /// Cisza.
    Quiet,
    /// Szum (wentylator, ulica, muzyka cicho).
    Noise,
    /// Głośniki (echo TTS / TV).
    Speakers,
    /// Słuchawki.
    Headphones,
}

/// Mikrofon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mic {
    /// USB / przewodowy.
    Usb,
    /// Wbudowany (laptop).
    Builtin,
    /// Mikrofon słuchawek.
    Headset,
    /// Wirtualny (próbka syntetyczna).
    Virtual,
}

/// Warunki nagrania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Conditions {
    /// Maszyna.
    pub machine: Machine,
    /// Otoczenie.
    pub environment: Environment,
    /// Mikrofon.
    pub mic: Mic,
}

/// Fragment pliku (wiele wypowiedzi w jednym nagraniu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    /// Początek (ms).
    pub start_ms: u64,
    /// Koniec (ms, wyłącznie).
    pub end_ms: u64,
}

/// Przepis próbki syntetycznej (CI — audio generowane, nie trzymane w gicie).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SynthSpec {
    /// Ziarno.
    pub seed: u64,
    /// Cisza przed mową (ms).
    pub lead_ms: u64,
    /// Długość mowy (ms).
    pub speech_ms: u64,
    /// Cisza po mowie (ms).
    pub tail_ms: u64,
}

/// Co mówiła agentka w chwili nagrania (typy 8–9 nagrywane na tle TTS): pozwala runnerowi offline
/// policzyć usłyszany prefiks tym samym automatem co potok.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TtsContext {
    /// Tekst odtwarzanej wypowiedzi.
    pub text: String,
    /// Długość audio wypowiedzi (ms).
    pub audio_ms: u64,
    /// Ile ms wypowiedzi było już odtworzone na początku pozycji (pliku / segmentu).
    #[serde(default)]
    pub played_at_start_ms: u64,
}

/// Pozycja manifestu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ManifestEntry {
    /// Identyfikator (unikalny, `[a-z0-9._-]`).
    pub id: String,
    /// Plik WAV względem katalogu korpusu (`16k/<maszyna>/<data>/t<NN>-….wav`).
    pub audio: String,
    /// Rodzaj nagrania.
    pub kind: ItemKind,
    /// Podział.
    pub split: Split,
    /// Warunki.
    pub conditions: Conditions,
    /// Transkrypcja referencyjna (liczby słownie, jak wypowiedziane; wtrącenia EN w oryginale).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<String>,
    /// Fragment pliku.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment: Option<Segment>,
    /// Komenda oczekiwana (typ 7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<CommandKind>,
    /// Początek słowa komendy w pliku (ms) — punkt odniesienia reakcji < 300 ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onset_ms: Option<u64>,
    /// Klasa intencji przerwania (typ 8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<InterruptIntent>,
    /// Prawdziwa liczba usłyszanych słów przed przerwaniem (typ 8, z odsłuchu/loopbacku).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heard_words: Option<usize>,
    /// Wypowiedź agentki w tle (typy 8–9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tts: Option<TtsContext>,
    /// Przepis próbki syntetycznej.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synth: Option<SynthSpec>,
    /// Uwagi (co szumiało, odległość…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Wczytuje manifest NDJSON (puste linie i linie `#` pomijane); błędy z numerem linii.
pub fn parse_manifest(text: &str) -> Result<Vec<ManifestEntry>, Vec<String>> {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match serde_json::from_str::<ManifestEntry>(line) {
            Ok(e) => out.push(e),
            Err(e) => errors.push(format!("linia {}: {e}", i + 1)),
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// Reguły formatu: unikalne id, ścieżki względne `.wav`, wymagane pola per rodzaj, brak
/// przecieku pliku między dev i test. Zwraca wszystkie naruszenia.
pub fn validate_manifest(entries: &[ManifestEntry]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut ids = BTreeSet::new();
    let mut splits: BTreeMap<&str, Split> = BTreeMap::new();
    for e in entries {
        let id = e.id.as_str();
        let id_ok = !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c));
        if !id_ok {
            errors.push(format!("{id}: niepoprawny identyfikator"));
        }
        if !ids.insert(id) {
            errors.push(format!("{id}: powtórzony identyfikator"));
        }
        let path = Path::new(&e.audio);
        if path.is_absolute() || e.audio.contains("..") || !e.audio.ends_with(".wav") {
            errors.push(format!("{id}: audio musi być względną ścieżką do .wav"));
        }
        if *splits.entry(e.audio.as_str()).or_insert(e.split) != e.split {
            errors.push(format!("{id}: plik {} jednocześnie w dev i test", e.audio));
        }
        let needs_text =
            e.kind.is_wer() || matches!(e.kind, ItemKind::Command | ItemKind::Backchannel);
        if needs_text && e.transcript.as_deref().is_none_or(|t| t.trim().is_empty()) {
            errors.push(format!("{id}: brak transkrypcji referencyjnej"));
        }
        if e.kind == ItemKind::Command && e.command.is_none() {
            errors.push(format!("{id}: komenda bez etykiety `command`"));
        }
        if e.kind == ItemKind::Interruption && e.intent.is_none() {
            errors.push(format!("{id}: przerwanie bez etykiety `intent`"));
        }
        if e.tts
            .as_ref()
            .is_some_and(|t| t.text.trim().is_empty() || t.audio_ms == 0)
        {
            errors.push(format!("{id}: `tts` bez tekstu lub długości"));
        }
        if e.segment.is_some_and(|s| s.end_ms <= s.start_ms) {
            errors.push(format!("{id}: segment pusty lub odwrócony"));
        }
        if (e.conditions.machine == Machine::Synthetic) != e.synth.is_some() {
            errors.push(format!(
                "{id}: `synth` tylko i zawsze dla maszyny `synthetic`"
            ));
        }
    }
    errors
}

/// JSON Schema pozycji manifestu (`evals/F2/manifest.schema.json`).
pub fn manifest_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(ManifestEntry)).unwrap_or_default()
}
