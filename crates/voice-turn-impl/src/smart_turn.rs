//! [`SmartTurnModel`]: koniec wypowiedzi oceniany na dźwięku modelem Smart Turn v3.2. Model liczy
//! osobny proces `alfa-smart-turn` (klient: `turn_sidecar`). Bez dźwięku, przy zbyt krótkim
//! nagraniu albo po błędzie procesu ocenę przejmuje model tekstowy.

use voice_turn_contract::{TurnError, TurnModel, TurnModelInput};

use crate::model::HeuristicTurnModel;
use crate::turn_sidecar::{SidecarCfg, SidecarScorer};

/// Nazwa pliku modelu.
pub const MODEL_FILE: &str = "smart-turn-v3.2-cpu.onnx";
/// Częstotliwość próbkowania, której oczekuje model.
pub const SAMPLE_RATE: u32 = 16_000;
/// Okno modelu: ostatnie 8 s.
pub const WINDOW_SAMPLES: usize = 128_000;
/// Najkrótsze nagranie, które ma sens oceniać modelem (0,2 s).
const MIN_SAMPLES: usize = 3_200;
/// Co ile ciszy ocena jest powtarzana (wartość z Delty AI).
pub const REPROBE_MS: u64 = 350;

pub(crate) fn model_err(reason: impl Into<String>) -> TurnError {
    TurnError::Model {
        reason: reason.into(),
    }
}

/// Ocena nagrania 16 kHz mono: prawdopodobieństwo końca wypowiedzi.
pub trait AudioScorer: Send + Sync {
    /// Ocena próbek.
    fn score(&self, samples: &[f32]) -> Result<f32, TurnError>;
}

/// Liczniki pracy modelu (do podglądu w Ustawieniach).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TurnStatsSnapshot {
    /// Udane oceny.
    pub scored: u32,
    /// Oceny nieudane (wtedy odpowiedział model tekstowy).
    pub failed: u32,
    /// Ostatnie prawdopodobieństwo końca wypowiedzi.
    pub last_probability: Option<f32>,
    /// Czas ostatniej oceny (ms).
    pub last_ms: Option<u32>,
}

/// Współdzielony uchwyt liczników.
#[derive(Debug, Clone, Default)]
pub struct TurnStats(std::sync::Arc<std::sync::Mutex<TurnStatsSnapshot>>);

impl TurnStats {
    /// Bieżące wartości.
    pub fn snapshot(&self) -> TurnStatsSnapshot {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn update(&self, change: impl FnOnce(&mut TurnStatsSnapshot)) {
        change(
            &mut self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }
}

/// Model końca wypowiedzi: dźwięk ocenia [`AudioScorer`], a bez dźwięku, przy zbyt krótkim nagraniu
/// albo po błędzie — model tekstowy.
pub struct SmartTurnModel {
    scorer: Box<dyn AudioScorer>,
    text: HeuristicTurnModel,
    stats: TurnStats,
}

impl std::fmt::Debug for SmartTurnModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmartTurnModel").finish_non_exhaustive()
    }
}

impl SmartTurnModel {
    /// Model z podanym źródłem ocen.
    pub fn new(scorer: Box<dyn AudioScorer>) -> Self {
        Self {
            scorer,
            text: HeuristicTurnModel,
            stats: TurnStats::default(),
        }
    }

    /// Liczniki zapisywane we wskazanym uchwycie (np. trzymanym przez okno Ustawień).
    #[must_use]
    pub fn with_stats(mut self, stats: TurnStats) -> Self {
        self.stats = stats;
        self
    }

    /// Uchwyt liczników.
    pub fn stats(&self) -> TurnStats {
        self.stats.clone()
    }

    /// Model z procesem `alfa-smart-turn`.
    pub fn sidecar(cfg: SidecarCfg) -> Self {
        Self::new(Box::new(SidecarScorer::new(cfg)))
    }
}

impl TurnModel for SmartTurnModel {
    fn name(&self) -> &str {
        "smart-turn-v3.2"
    }

    fn end_probability(&self, input: &TurnModelInput<'_>) -> Result<f32, TurnError> {
        if let Some(tail) = input.audio
            && tail.sample_rate == SAMPLE_RATE
            && tail.samples.len() >= MIN_SAMPLES
        {
            let started = std::time::Instant::now();
            match self.scorer.score(tail.samples) {
                Ok(p) => {
                    let ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
                    self.stats.update(|s| {
                        s.scored = s.scored.saturating_add(1);
                        s.last_probability = Some(p);
                        s.last_ms = Some(ms);
                    });
                    return Ok(p);
                }
                Err(_) => self.stats.update(|s| s.failed = s.failed.saturating_add(1)),
            }
        }
        self.text.end_probability(input)
    }

    fn reprobe_ms(&self) -> Option<u64> {
        Some(REPROBE_MS)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use voice_turn_contract::AudioTail;

    use super::*;
    use crate::model::{P_CLEAR, P_NEUTRAL};

    struct Fixed(Result<f32, ()>, std::sync::Arc<AtomicUsize>);

    impl AudioScorer for Fixed {
        fn score(&self, _: &[f32]) -> Result<f32, TurnError> {
            self.1.fetch_add(1, Ordering::SeqCst);
            self.0.map_err(|()| model_err("zepsute"))
        }
    }

    fn model(result: Result<f32, ()>) -> (SmartTurnModel, std::sync::Arc<AtomicUsize>) {
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        (
            SmartTurnModel::new(Box::new(Fixed(result, calls.clone()))),
            calls,
        )
    }

    fn ask(m: &SmartTurnModel, samples: &[f32], rate: u32, text: Option<&str>) -> f32 {
        m.end_probability(&TurnModelInput {
            audio: Some(AudioTail {
                samples,
                sample_rate: rate,
            }),
            partial_text: text,
            silence_ms: 250,
        })
        .unwrap()
    }

    #[test]
    fn audio_goes_to_the_model() {
        let (m, calls) = model(Ok(0.93));
        assert!((ask(&m, &[0.1; 16_000], 16_000, None) - 0.93).abs() < 1e-6);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(m.name(), "smart-turn-v3.2");
    }

    #[test]
    fn short_or_wrong_rate_audio_uses_text() {
        let (m, calls) = model(Ok(0.93));
        assert_eq!(ask(&m, &[0.1; 100], 16_000, Some("Dziękuję.")), P_CLEAR);
        assert_eq!(ask(&m, &[0.1; 48_000], 48_000, None), P_NEUTRAL);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let none = m
            .end_probability(&TurnModelInput {
                audio: None,
                partial_text: None,
                silence_ms: 0,
            })
            .unwrap();
        assert_eq!(none, P_NEUTRAL);
    }

    #[test]
    fn failing_model_falls_back_to_text() {
        let (m, calls) = model(Err(()));
        assert_eq!(ask(&m, &[0.1; 16_000], 16_000, Some("Dziękuję.")), P_CLEAR);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let stats = m.stats().snapshot();
        assert_eq!((stats.scored, stats.failed), (0, 1));
        assert_eq!(stats.last_probability, None);
    }

    #[test]
    fn stats_count_scores_in_a_shared_handle() {
        let shared = TurnStats::default();
        let (m, _) = model(Ok(0.93));
        let m = m.with_stats(shared.clone());
        ask(&m, &[0.1; 16_000], 16_000, None);
        ask(&m, &[0.1; 16_000], 16_000, None);
        ask(&m, &[0.1; 100], 16_000, None); // za krótkie — bez oceny modelem
        let stats = shared.snapshot();
        assert_eq!((stats.scored, stats.failed), (2, 0));
        assert!((stats.last_probability.unwrap() - 0.93).abs() < 1e-6);
        assert!(stats.last_ms.is_some());
    }
}
