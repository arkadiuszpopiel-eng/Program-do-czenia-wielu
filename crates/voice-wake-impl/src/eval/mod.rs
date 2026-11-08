//! Runner FAR/FRR słów wywoławczych (ACCEPTANCE F5-05/F5-06, `evals/F5/voice/`): manifest NDJSON,
//! strumieniowe audio, model KWS raz na nagranie (wyniki co krok) → przegląd progów tym samym
//! detektorem co w potoku (`voice_wake_contract::eval`), rekomendacja progu, wynik per pozycja.
//! Działa wyłącznie lokalnie — audio i wyniki nie opuszczają maszyny.

pub mod audio;
pub mod manifest;

use std::path::Path;

use serde::{Deserialize, Serialize};
use voice_wake_contract::eval::{ScoredItem, ScoredKind, SweepPoint, evaluate, recommend, sweep};
use voice_wake_contract::{
    KeywordScorer, KwsParams, WakeWordCfg, WakeWordDetector, WakeWordListener,
};

pub use manifest::{
    Conditions, Segment, Split, SynthRecipe, WakeItem, WakeItemKind, parse_manifest,
    validate_manifest,
};

/// Wykrycie w wyniku pozycji.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HitRecord {
    /// Czas (ms od początku nagrania).
    pub at_ms: u64,
    /// Adresatka.
    pub persona: String,
    /// Wynik.
    pub score: f32,
}

/// Wynik pozycji przy wybranym progu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemOutcome {
    /// Identyfikator.
    pub id: String,
    /// Rodzaj.
    pub kind: WakeItemKind,
    /// Długość (ms).
    pub duration_ms: u64,
    /// Najwyższy wynik modelu (dowolna etykieta).
    pub max_score: f32,
    /// Wykrycia.
    pub hits: Vec<HitRecord>,
}

/// Podsumowanie przebiegu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// Etykiety modelu.
    pub labels: Vec<String>,
    /// Próg oceny.
    pub threshold: f32,
    /// Wynik przy progu oceny.
    pub at_threshold: SweepPoint,
    /// Przegląd progów.
    pub sweep: Vec<SweepPoint>,
    /// Najlepszy próg z FAR ≤ 1/dzień.
    pub recommended: Option<SweepPoint>,
    /// F5-05: FAR ≤ 1/dzień.
    pub f5_05_far_ok: bool,
    /// F5-06: FRR ≤ 5%.
    pub f5_06_frr_ok: bool,
    /// Liczności wystarczają (≥ 24 h tła, ≥ 200 pozytywów).
    pub sufficient: bool,
}

/// Przelicza nagrania modelem (jeden model, reset między nagraniami). `audio_root` — katalog
/// korpusu; pozycje syntetyczne nie potrzebują plików.
pub fn score_items(
    items: &[&WakeItem],
    audio_root: &Path,
    cfg: &WakeWordCfg,
    params: KwsParams,
    scorer: Box<dyn KeywordScorer>,
) -> Result<Vec<ScoredItem>, String> {
    let mut scorer = Some(scorer);
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        let mut s = scorer.take().ok_or("model utracony")?;
        s.reset();
        let mut listener = WakeWordListener::new(cfg, params, s).map_err(|e| e.to_string())?;
        let mut scores = Vec::new();
        let duration_ms = if let Some(recipe) = &it.synth {
            let pcm = audio::synth(recipe);
            for block in pcm.chunks(audio::RATE as usize) {
                scores.extend(listener.push_scores(block).map_err(|e| e.to_string())?);
            }
            pcm.len() as u64 * 1000 / u64::from(audio::RATE)
        } else {
            let rel = it.audio.as_deref().ok_or("brak audio")?;
            let mut wav = audio::WavReader::open(&audio_root.join(rel))?;
            loop {
                let block = wav.next_block(audio::RATE as usize)?;
                if block.is_empty() {
                    break;
                }
                scores.extend(listener.push_scores(&block).map_err(|e| e.to_string())?);
            }
            wav.duration_ms
        };
        scorer = Some(listener.into_scorer());
        let kind = match it.kind {
            WakeItemKind::WakeBackground => ScoredKind::Background,
            WakeItemKind::WakePositive => ScoredKind::Positive {
                persona: it.persona_id().ok_or("pozytyw bez persony")?,
                window: it.segment.map(|s| (s.start_ms, s.end_ms)),
            },
        };
        out.push(ScoredItem {
            id: it.id.clone(),
            kind,
            duration_ms,
            scores,
        });
    }
    Ok(out)
}

/// Podsumowanie: przegląd progów, rekomendacja, ocena przy `threshold`.
pub fn summarize(
    scored: &[ScoredItem],
    cfg: &WakeWordCfg,
    labels: &[String],
    params: KwsParams,
    threshold: f32,
    thresholds: &[f32],
) -> Result<Summary, String> {
    let points = sweep(scored, cfg, labels, params, thresholds).map_err(|e| e.to_string())?;
    let at = evaluate(scored, cfg, labels, params, threshold).map_err(|e| e.to_string())?;
    Ok(Summary {
        labels: labels.to_vec(),
        threshold,
        f5_05_far_ok: at.far_per_day() <= voice_wake_contract::eval::FAR_PER_DAY_MAX,
        f5_06_frr_ok: at.frr() <= voice_wake_contract::eval::FRR_MAX,
        sufficient: at.sufficient(),
        recommended: recommend(&points).cloned(),
        sweep: points,
        at_threshold: at,
    })
}

/// Wyniki per pozycja przy progu (NDJSON `results`).
pub fn outcomes(
    scored: &[ScoredItem],
    items: &[&WakeItem],
    cfg: &WakeWordCfg,
    labels: &[String],
    params: KwsParams,
    threshold: f32,
) -> Result<Vec<ItemOutcome>, String> {
    let cfg = WakeWordCfg {
        threshold,
        ..cfg.clone()
    };
    scored
        .iter()
        .zip(items)
        .map(|(s, it)| {
            let mut det = WakeWordDetector::new(&cfg, labels, params).map_err(|e| e.to_string())?;
            Ok(ItemOutcome {
                id: s.id.clone(),
                kind: it.kind,
                duration_ms: s.duration_ms,
                max_score: s
                    .scores
                    .iter()
                    .flat_map(|k| k.scores.iter().copied())
                    .fold(0.0, f32::max),
                hits: s
                    .scores
                    .iter()
                    .filter_map(|k| det.push(k))
                    .map(|h| HitRecord {
                        at_ms: h.at_ms,
                        persona: h.persona.to_string(),
                        score: h.score,
                    })
                    .collect(),
            })
        })
        .collect()
}

/// JSON Schema pozycji manifestu (`evals/F5/voice/wake-manifest.schema.json`).
pub fn manifest_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(WakeItem)).unwrap_or_default()
}
