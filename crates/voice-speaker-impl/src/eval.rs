//! Runner EER weryfikacji mówcy (ACCEPTANCE F5-07/F5-08, `evals/F5/voice/README.md` §2):
//! manifest NDJSON (rejestracja właściciela + próby właściciela i obcych: Common Voice PL, TTS),
//! WAV 16 kHz mono albo przepis syntetyczny (CI), rejestracja w **profilu w pamięci** (runner
//! nigdy nie dotyka zapisanego profilu użytkownika), wyniki prób → EER, FAR/FRR przy progach,
//! próg dla FAR ≤ 0,1%. Lokalnie, bez sieci.

use std::collections::BTreeSet;
use std::path::{Component, Path};
use std::sync::Mutex;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use voice_audio_contract::synth::{SpeechParams, synthetic_speech};
use voice_audio_contract::wav::decode_wav;
use voice_speaker_contract::eer::{EerReport, report};
use voice_speaker_contract::{
    EmbeddingModel, Profile, ProfileStore, SPEAKER_RATE, SpeakerCfg, SpeakerEngine, SpeakerError,
    SpeakerVerifier,
};

/// Rola pozycji.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Wypowiedź rejestracji właściciela (typ 11 protokołu nagrań).
    Enroll,
    /// Próba weryfikacji (właściciel albo obcy).
    Trial,
}

/// Źródło nagrania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Korpus własny (bramka #3).
    Own,
    /// Common Voice PL (obce głosy; licencja CC0).
    CommonVoice,
    /// Synteza TTS (obce głosy, także próby podszycia).
    Tts,
    /// Próbka syntetyczna CI.
    Synthetic,
}

/// Przepis próbki syntetycznej: ton podstawowy i ziarno.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SynthVoice {
    /// F0 (Hz).
    pub f0: f32,
    /// Ziarno.
    pub seed: u64,
    /// Długość (s).
    pub secs: f32,
}

/// Pozycja manifestu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SpeakerItem {
    /// Unikalny identyfikator `[a-z0-9._:-]`.
    pub id: String,
    /// WAV 16 kHz mono (względnie wobec `--audio-root`, bez `..`).
    #[serde(default)]
    pub audio: Option<String>,
    /// Przepis syntetyczny (zamiast `audio`).
    #[serde(default)]
    pub synth: Option<SynthVoice>,
    /// `owner` albo identyfikator obcego mówcy (np. `cv:<hash>`, `tts:pocket-beta`).
    pub speaker: String,
    /// Rola.
    pub role: Role,
    /// `dev` / `test`.
    pub split: String,
    /// Źródło.
    pub source: Source,
    /// Uwagi (szept, odległość).
    #[serde(default)]
    pub note: Option<String>,
}

/// Wynik próby.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrialOutcome {
    /// Identyfikator.
    pub id: String,
    /// Mówca.
    pub speaker: String,
    /// Czy właściciel.
    pub genuine: bool,
    /// Wynik (kosinus) albo `None`, gdy wypowiedź odrzucona (za krótka/cicha).
    pub score: Option<f32>,
}

/// Parsuje NDJSON (puste linie i `#…` pomijane).
pub fn parse_manifest(text: &str) -> Result<Vec<SpeakerItem>, String> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .map(|(i, l)| serde_json::from_str(l).map_err(|e| format!("linia {}: {e}", i + 1)))
        .collect()
}

/// Walidacja (identyfikatory, ścieżki, rejestracja tylko właściciela, ≥ 3 wypowiedzi rejestracji).
pub fn validate_manifest(items: &[SpeakerItem]) -> Vec<String> {
    let mut problems = Vec::new();
    let mut ids = BTreeSet::new();
    for it in items {
        let p = |m: &str| format!("{}: {m}", it.id);
        if it.id.is_empty()
            || !it
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._:-".contains(c))
        {
            problems.push(p("identyfikator musi być [a-z0-9._:-]"));
        }
        if !ids.insert(it.id.as_str()) {
            problems.push(p("identyfikator powtórzony"));
        }
        let path_ok = |a: &str| {
            a.ends_with(".wav")
                && !a.contains(['\\', ':'])
                && Path::new(a)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
        };
        match (&it.audio, &it.synth) {
            (Some(a), None) if !path_ok(a) => {
                problems.push(p("audio: ścieżka względna .wav bez `..`"))
            }
            (Some(_), None) | (None, Some(_)) => {}
            _ => problems.push(p("dokładnie jedno z `audio` i `synth`")),
        }
        if it.role == Role::Enroll && it.speaker != "owner" {
            problems.push(p("rejestracja tylko głosu właściciela"));
        }
        if !["dev", "test"].contains(&it.split.as_str()) {
            problems.push(p("split: dev|test"));
        }
    }
    let enroll = items.iter().filter(|i| i.role == Role::Enroll).count();
    if enroll < voice_speaker_contract::MIN_ENROLL_UTTERANCES {
        problems.push(format!("rejestracja: {enroll} wypowiedzi < 3"));
    }
    problems
}

/// Audio pozycji (16 kHz mono).
pub fn load_audio(it: &SpeakerItem, root: &Path) -> Result<Vec<f32>, String> {
    if let Some(s) = &it.synth {
        let p = SpeechParams {
            f0: s.f0,
            seed: s.seed,
            ..SpeechParams::default()
        };
        return Ok(synthetic_speech(SPEAKER_RATE, s.secs, p));
    }
    let rel = it.audio.as_deref().ok_or("brak audio")?;
    let bytes = std::fs::read(root.join(rel)).map_err(|e| format!("{rel}: {e}"))?;
    let (pcm, fmt) = decode_wav(&bytes).map_err(|e| format!("{rel}: {e}"))?;
    if fmt.sample_rate != SPEAKER_RATE || fmt.channels != 1 {
        return Err(format!(
            "{rel}: wymagane 16 kHz mono (ffmpeg -ac 1 -ar 16000)"
        ));
    }
    Ok(pcm)
}

/// Profil w pamięci (runner nie dotyka profilu użytkownika).
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<Profile>>);

impl ProfileStore for MemoryStore {
    fn load(&self) -> Result<Option<Profile>, SpeakerError> {
        Ok(self.0.lock().unwrap_or_else(|p| p.into_inner()).clone())
    }
    fn save(&self, profile: &Profile) -> Result<(), SpeakerError> {
        *self.0.lock().unwrap_or_else(|p| p.into_inner()) = Some(profile.clone());
        Ok(())
    }
    fn delete(&self) -> Result<bool, SpeakerError> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .is_some())
    }
}

/// Przebieg: rejestracja z pozycji `enroll`, potem próby z podziału `split` → raport EER.
pub fn run<M: EmbeddingModel>(
    items: &[SpeakerItem],
    root: &Path,
    model: M,
    cfg: SpeakerCfg,
    split: &str,
) -> Result<(EerReport, Vec<TrialOutcome>), String> {
    let engine = SpeakerEngine::new(model, Box::new(MemoryStore::default()), cfg)
        .map_err(|e| e.to_string())?;
    engine.begin_enrollment().map_err(|e| e.to_string())?;
    for it in items.iter().filter(|i| i.role == Role::Enroll) {
        engine
            .add_enrollment(&load_audio(it, root)?)
            .map_err(|e| format!("{}: {e}", it.id))?;
    }
    engine.finish_enrollment().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for it in items
        .iter()
        .filter(|i| i.role == Role::Trial && i.split == split)
    {
        let score = match engine.verify(&load_audio(it, root)?) {
            Ok(v) => Some(v.score),
            Err(SpeakerError::TooShort { .. } | SpeakerError::TooQuiet) => None,
            Err(e) => return Err(format!("{}: {e}", it.id)),
        };
        out.push(TrialOutcome {
            id: it.id.clone(),
            speaker: it.speaker.clone(),
            genuine: it.speaker == "owner",
            score,
        });
    }
    // Wypowiedź odrzucona przed oceną (za krótka/cicha) = odrzucenie (dla właściciela błąd FRR).
    let pick = |g: bool| -> Vec<f32> {
        out.iter()
            .filter(|t| t.genuine == g)
            .map(|t| t.score.unwrap_or(-1.0))
            .collect()
    };
    let r = report(
        &pick(true),
        &pick(false),
        cfg.threshold_standard,
        cfg.threshold_strict,
    );
    Ok((r, out))
}

/// JSON Schema pozycji manifestu (`evals/F5/voice/speaker-manifest.schema.json`).
pub fn manifest_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(SpeakerItem)).unwrap_or_default()
}
