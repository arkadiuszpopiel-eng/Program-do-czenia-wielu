//! Polityka Ulepszacza — dana przez kompozycję (klucze `improver.*`, których Ulepszacz nie może
//! zmienić: prefiks zakazany w strażniku). Brak metody zmiany w trakcie działania.

use evals_contract::{Direction, MIN_GATE_REPEATS, SuiteId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ports::ImproverError;
use crate::ring::Ring;

/// Metryka pilnowana po wdrożeniu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WatchedMetric {
    /// Nazwa (jak w [`crate::MetricsSnapshot::metrics`]).
    pub name: String,
    /// Kierunek.
    pub direction: Direction,
}

/// Polityka.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ImproverPolicy {
    /// Automatyczne wdrażanie R0 zawężających/bezpiecznych po bramce.
    pub auto_deploy_r0: bool,
    /// Powtórzenia w bramce (≥ 5).
    pub repeats: u32,
    /// Zestaw bramki dla R0.
    pub suite_r0: String,
    /// Zestaw bramki dla R1.
    pub suite_r1: String,
    /// Zestaw bramki dla R2.
    pub suite_r2: String,
    /// Maksymalna liczba zmian w propozycji.
    pub max_changes_per_proposal: usize,
    /// Maksymalna liczba propozycji na dobę.
    pub max_proposals_per_day: u32,
    /// Maksymalna liczba aktywnych wdrożeń (w nadzorze).
    pub max_active_deployments: usize,
    /// Okno nadzoru po wdrożeniu (ms).
    pub watch_window_ms: u64,
    /// Tolerancja regresji (bezwzględna).
    pub regression_tolerance: f64,
    /// Metryki pilnowane po wdrożeniu.
    pub watch_metrics: Vec<WatchedMetric>,
    /// Wychładzanie klucza po rollbacku (ms).
    pub cooldown_after_rollback_ms: u64,
    /// Pracuj tylko przy bezczynności użytkownika.
    pub require_idle: bool,
}

impl Default for ImproverPolicy {
    fn default() -> Self {
        let watch = |name: &str, direction| WatchedMetric {
            name: name.to_owned(),
            direction,
        };
        Self {
            auto_deploy_r0: true,
            repeats: MIN_GATE_REPEATS,
            suite_r0: "f8-improver-r0".into(),
            suite_r1: "f8-improver-r1".into(),
            suite_r2: "f8-improver-r2".into(),
            max_changes_per_proposal: 10,
            max_proposals_per_day: 20,
            max_active_deployments: 5,
            watch_window_ms: 24 * 60 * 60 * 1000,
            regression_tolerance: 0.02,
            watch_metrics: vec![
                watch("pass_rate", Direction::HigherIsBetter),
                watch("task_success_rate", Direction::HigherIsBetter),
                watch("corrections_per_hour", Direction::LowerIsBetter),
                watch("false_interruptions_per_hour", Direction::LowerIsBetter),
            ],
            cooldown_after_rollback_ms: 7 * 24 * 60 * 60 * 1000,
            require_idle: true,
        }
    }
}

impl ImproverPolicy {
    /// Walidacja (N ≥ 5, tolerancja w [0, 1], limity > 0, zestawy poprawne).
    pub fn validate(&self) -> Result<(), ImproverError> {
        let bad = |m: &str| Err(ImproverError::Policy(m.to_owned()));
        if self.repeats < MIN_GATE_REPEATS {
            return bad("repeats < 5 (PLAN §12.4)");
        }
        if !(0.0..=1.0).contains(&self.regression_tolerance) {
            return bad("regression_tolerance poza [0, 1]");
        }
        if self.max_changes_per_proposal == 0
            || self.max_proposals_per_day == 0
            || self.max_active_deployments == 0
            || self.watch_window_ms == 0
        {
            return bad("limity muszą być > 0");
        }
        for ring in [Ring::R0, Ring::R1, Ring::R2] {
            self.suite_for(ring)?;
        }
        Ok(())
    }

    /// Zestaw bramki dla pierścienia (R3 i Jądro — brak).
    pub fn suite_for(&self, ring: Ring) -> Result<SuiteId, ImproverError> {
        let name = match ring {
            Ring::R0 => &self.suite_r0,
            Ring::R1 => &self.suite_r1,
            Ring::R2 => &self.suite_r2,
            Ring::R3 | Ring::Kernel => {
                return Err(ImproverError::Policy(format!("brak bramki dla {ring:?}")));
            }
        };
        SuiteId::new(name.clone()).map_err(|e| ImproverError::Policy(e.to_string()))
    }
}
