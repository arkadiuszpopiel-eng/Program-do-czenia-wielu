//! Metryki słów wywoławczych (ACCEPTANCE F5-05/F5-06): FAR na dobę na nagraniach tła i FRR na
//! pozytywach właściciela, z przeglądem progów (krzywa DET). Strumienie wyników modelu liczone
//! raz ([`crate::WakeWordListener::push_scores`]) i odtwarzane przez ten sam detektor co w potoku.

use personas_contract::PersonaId;
use serde::{Deserialize, Serialize};

use crate::detector::{KwsScores, WakeWordDetector};
use crate::words::KwsParams;
use crate::{WakeError, WakeWordCfg};

/// Próg F5-05: fałszywe wybudzenia na dobę.
pub const FAR_PER_DAY_MAX: f64 = 1.0;
/// Próg F5-06: odsetek nierozpoznanych pozytywów.
pub const FRR_MAX: f64 = 0.05;
/// Minimum nagrań tła (h) dla oceny F5-05.
pub const MIN_BACKGROUND_HOURS: f64 = 24.0;
/// Minimum pozytywów dla oceny F5-06.
pub const MIN_POSITIVES: usize = 200;

/// Rodzaj nagrania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScoredKind {
    /// Pozytyw: fraza wypowiedziana przez właściciela; oczekiwana adresatka.
    Positive {
        /// Oczekiwana persona.
        persona: PersonaId,
        /// Okno, w którym musi paść wykrycie (ms; `None` = całe nagranie).
        window: Option<(u64, u64)>,
    },
    /// Tło (TV, podcasty, rozmowy) — każde wykrycie to fałszywy alarm.
    Background,
}

/// Nagranie przeliczone przez model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredItem {
    /// Identyfikator pozycji manifestu.
    pub id: String,
    /// Rodzaj.
    pub kind: ScoredKind,
    /// Długość audio (ms).
    pub duration_ms: u64,
    /// Wyniki kroków modelu.
    pub scores: Vec<KwsScores>,
}

/// Wynik przy jednym progu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepPoint {
    /// Próg.
    pub threshold: f32,
    /// Pozytywy.
    pub positives: usize,
    /// Pozytywy wykryte z właściwą adresatką.
    pub detected: usize,
    /// Pozytywy wykryte z niewłaściwą adresatką.
    pub wrong_persona: usize,
    /// Godziny tła.
    pub background_hours: f64,
    /// Fałszywe wybudzenia na tle.
    pub false_alarms: usize,
    /// Identyfikatory przegapionych pozytywów (do odsłuchu).
    pub missed: Vec<String>,
}

impl SweepPoint {
    /// FRR (0–1); brak pozytywów = 1 (nie da się ocenić).
    pub fn frr(&self) -> f64 {
        if self.positives == 0 {
            1.0
        } else {
            1.0 - self.detected as f64 / self.positives as f64
        }
    }

    /// FAR na dobę; brak tła = nieskończoność.
    pub fn far_per_day(&self) -> f64 {
        if self.background_hours <= 0.0 {
            f64::INFINITY
        } else {
            self.false_alarms as f64 / self.background_hours * 24.0
        }
    }

    /// Spełnia progi F5-05 i F5-06 (bez sprawdzania liczności).
    pub fn passes(&self) -> bool {
        self.far_per_day() <= FAR_PER_DAY_MAX && self.frr() <= FRR_MAX
    }

    /// Liczności wystarczają do oceny akceptacyjnej (≥ 24 h tła, ≥ 200 pozytywów).
    pub fn sufficient(&self) -> bool {
        self.background_hours >= MIN_BACKGROUND_HOURS && self.positives >= MIN_POSITIVES
    }
}

/// Liczy punkt dla progu: każdy strumień przez świeży detektor.
pub fn evaluate(
    items: &[ScoredItem],
    cfg: &WakeWordCfg,
    labels: &[String],
    params: KwsParams,
    threshold: f32,
) -> Result<SweepPoint, WakeError> {
    let cfg = WakeWordCfg {
        threshold,
        ..cfg.clone()
    };
    let mut p = SweepPoint {
        threshold,
        positives: 0,
        detected: 0,
        wrong_persona: 0,
        background_hours: 0.0,
        false_alarms: 0,
        missed: Vec::new(),
    };
    for item in items {
        let mut det = WakeWordDetector::new(&cfg, labels, params)?;
        let hits: Vec<_> = item.scores.iter().filter_map(|s| det.push(s)).collect();
        match &item.kind {
            ScoredKind::Background => {
                p.background_hours += item.duration_ms as f64 / 3_600_000.0;
                p.false_alarms += hits.len();
            }
            ScoredKind::Positive { persona, window } => {
                p.positives += 1;
                let inside = |t: u64| window.is_none_or(|(a, b)| t >= a && t <= b + 500);
                let in_window: Vec<_> = hits.iter().filter(|h| inside(h.at_ms)).collect();
                if in_window.iter().any(|h| &h.persona == persona) {
                    p.detected += 1;
                } else {
                    if !in_window.is_empty() {
                        p.wrong_persona += 1;
                    }
                    p.missed.push(item.id.clone());
                }
            }
        }
    }
    Ok(p)
}

/// Przegląd progów (rosnąco).
pub fn sweep(
    items: &[ScoredItem],
    cfg: &WakeWordCfg,
    labels: &[String],
    params: KwsParams,
    thresholds: &[f32],
) -> Result<Vec<SweepPoint>, WakeError> {
    let mut ts: Vec<f32> = thresholds.to_vec();
    ts.sort_by(f32::total_cmp);
    ts.dedup();
    ts.iter()
        .map(|t| evaluate(items, cfg, labels, params, *t))
        .collect()
}

/// Najniższy próg spełniający FAR ≤ 1/dzień (najmniejszy FRR przy dopuszczalnym FAR).
pub fn recommend(points: &[SweepPoint]) -> Option<&SweepPoint> {
    points
        .iter()
        .filter(|p| p.far_per_day() <= FAR_PER_DAY_MAX)
        .min_by(|a, b| a.frr().total_cmp(&b.frr()))
}

/// Domyślna siatka progów 0,30–0,95 co 0,05.
pub fn default_thresholds() -> Vec<f32> {
    (6..=19).map(|i| i as f32 * 0.05).collect()
}
