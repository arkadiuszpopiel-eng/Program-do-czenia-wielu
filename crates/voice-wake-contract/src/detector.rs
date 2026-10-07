//! Deterministyczny detektor słów wywoławczych na wynikach modelu KWS: próg z histerezą
//! (uzbrojenie dopiero po spadku poniżej `próg − histereza`), `min_hits` kolejnych kroków ≥ próg
//! i okno odporności po wykryciu. Wspólny dla `-impl` (model ONNX), `-fake` (skrypt) i runnera
//! FAR/FRR — ten sam rdzeń liczy metryki i działa w potoku.

use personas_contract::PersonaId;
use serde::{Deserialize, Serialize};

use crate::words::KwsParams;
use crate::{WakeError, WakeWordCfg};

/// Wyniki modelu w jednym kroku (po jednej wartości 0–1 na etykietę).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KwsScores {
    /// Czas końca okna analizy (ms od początku strumienia).
    pub at_ms: u64,
    /// Wyniki (kolejność = etykiety modelu).
    pub scores: Vec<f32>,
}

/// Wykrycie frazy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WakeWordHit {
    /// Fraza z konfiguracji („Hej Delta”).
    pub phrase: String,
    /// Adresatka.
    pub persona: PersonaId,
    /// Najwyższy wynik w serii trafień.
    pub score: f32,
    /// Czas wykrycia (ms).
    pub at_ms: u64,
}

impl WakeWordHit {
    /// Wynik w promilach (do zdarzeń bez liczb zmiennoprzecinkowych).
    pub fn score_permille(&self) -> u16 {
        (self.score.clamp(0.0, 1.0) * 1000.0).round() as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LabelState {
    armed: bool,
    run: u32,
    peak: f32,
}

/// Detektor (stan per etykieta).
#[derive(Debug, Clone)]
pub struct WakeWordDetector {
    threshold: f32,
    params: KwsParams,
    /// Etykieta modelu → (fraza, persona) albo `None`, gdy etykiety nie ma w konfiguracji.
    map: Vec<Option<(String, PersonaId)>>,
    state: Vec<LabelState>,
    last_hit_ms: Option<u64>,
}

impl WakeWordDetector {
    /// Detektor dla etykiet modelu i konfiguracji fraz. Błąd, gdy żadna fraza nie ma etykiety.
    pub fn new(cfg: &WakeWordCfg, labels: &[String], params: KwsParams) -> Result<Self, WakeError> {
        cfg.validate()?;
        params.validate()?;
        let map: Vec<Option<(String, PersonaId)>> = labels
            .iter()
            .map(|l| cfg.persona_for(l).map(|(p, id)| (p.to_owned(), id.clone())))
            .collect();
        if map.iter().all(Option::is_none) {
            return Err(WakeError::InvalidConfig(format!(
                "model nie zna żadnej z fraz (etykiety: {labels:?})"
            )));
        }
        Ok(Self {
            threshold: cfg.threshold,
            params,
            state: vec![
                LabelState {
                    armed: true,
                    run: 0,
                    peak: 0.0,
                };
                map.len()
            ],
            map,
            last_hit_ms: None,
        })
    }

    /// Próg.
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Frazy z konfiguracji, których model nie zna (UI: „ten model nie wykryje …”).
    pub fn mapped_labels(&self) -> usize {
        self.map.iter().flatten().count()
    }

    /// Jeden krok modelu. Zwraca wykrycie (najwyżej jedno — najwyższy wynik wśród gotowych).
    pub fn push(&mut self, s: &KwsScores) -> Option<WakeWordHit> {
        let off = self.threshold - self.params.hysteresis;
        let refractory = self
            .last_hit_ms
            .is_some_and(|t| s.at_ms < t.saturating_add(self.params.refractory_ms));
        let mut best: Option<(usize, f32)> = None;
        for (i, st) in self.state.iter_mut().enumerate() {
            let score = s.scores.get(i).copied().unwrap_or(0.0);
            let score = if score.is_finite() { score } else { 0.0 };
            if score < off {
                st.armed = true;
                st.run = 0;
                st.peak = 0.0;
                continue;
            }
            if score < self.threshold {
                st.run = 0;
                continue;
            }
            st.run += 1;
            st.peak = st.peak.max(score);
            if st.armed
                && !refractory
                && st.run >= self.params.min_hits
                && self.map.get(i).is_some_and(Option::is_some)
                && best.is_none_or(|(_, b)| st.peak > b)
            {
                best = Some((i, st.peak));
            }
        }
        let (i, peak) = best?;
        self.last_hit_ms = Some(s.at_ms);
        for st in &mut self.state {
            st.armed = false;
            st.run = 0;
        }
        let (phrase, persona) = self.map.get(i).cloned().flatten()?;
        Some(WakeWordHit {
            phrase,
            persona,
            score: peak,
            at_ms: s.at_ms,
        })
    }

    /// Reset (nowy strumień; okno odporności też).
    pub fn reset(&mut self) {
        for st in &mut self.state {
            *st = LabelState {
                armed: true,
                run: 0,
                peak: 0.0,
            };
        }
        self.last_hit_ms = None;
    }
}
