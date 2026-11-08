//! Atrapy portów Strażniczki pamięci (deterministyczne):
//! - [`ScriptedConsolidator`] — kolejka odpowiedzi „modelu”, zapis otrzymanych wsadów;
//! - [`FixedBudget`] — stały werdykt budżetu tła, zapis sprawdzeń i rejestracji zużycia;
//! - [`FakeHost`] — sterowany stan maszyny (bateria, tryb gry, bezczynność, godzina).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use chrono::NaiveTime;
use memory_consolidation_contract::{
    BackgroundBudget, BudgetVerdict, ConsolidationBatch, ConsolidationError, Consolidator,
    ConsolidatorModel, ConsolidatorOutput, HostConditions, HostState, LlmUsage,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Skryptowany konsolidator: każde wywołanie zdejmuje następną odpowiedź (pusta kolejka → wynik
/// pusty); zapisuje wsady (testy: brak treści niezaufanej i prywatnej w zapytaniach do modelu).
pub struct ScriptedConsolidator {
    model: ConsolidatorModel,
    replies: Mutex<VecDeque<Result<ConsolidatorOutput, ConsolidationError>>>,
    batches: Mutex<Vec<ConsolidationBatch>>,
    estimate: Option<u64>,
}

impl ScriptedConsolidator {
    /// Model lokalny (koszt 0).
    pub fn local() -> Self {
        Self::with_model(ConsolidatorModel {
            provider: "lokalny".into(),
            model: "atrapa-4b".into(),
            local: true,
        })
    }

    /// Model chmurowy o podanym szacunku kosztu (mikro-USD).
    pub fn cloud(estimate_micro_usd: u64) -> Self {
        Self {
            estimate: Some(estimate_micro_usd),
            ..Self::with_model(ConsolidatorModel {
                provider: "chmura".into(),
                model: "atrapa-chmura".into(),
                local: false,
            })
        }
    }

    fn with_model(model: ConsolidatorModel) -> Self {
        Self {
            model,
            replies: Mutex::new(VecDeque::new()),
            batches: Mutex::new(Vec::new()),
            estimate: Some(0),
        }
    }

    /// Dodaje odpowiedź.
    pub fn push(&self, reply: Result<ConsolidatorOutput, ConsolidationError>) {
        lock(&self.replies).push_back(reply);
    }

    /// Otrzymane wsady.
    pub fn batches(&self) -> Vec<ConsolidationBatch> {
        lock(&self.batches).clone()
    }
}

#[async_trait]
impl Consolidator for ScriptedConsolidator {
    fn model(&self) -> ConsolidatorModel {
        self.model.clone()
    }

    fn estimate_micro_usd(&self, _batch: &ConsolidationBatch) -> Option<u64> {
        self.estimate
    }

    async fn consolidate(
        &self,
        batch: &ConsolidationBatch,
    ) -> Result<ConsolidatorOutput, ConsolidationError> {
        lock(&self.batches).push(batch.clone());
        let mut out = lock(&self.replies)
            .pop_front()
            .unwrap_or_else(|| Ok(ConsolidatorOutput::default()))?;
        if out.usage.is_none() {
            out.usage = Some(LlmUsage {
                provider: self.model.provider.clone(),
                model: self.model.model.clone(),
                input_tokens: 100,
                output_tokens: 50,
                cost_micro_usd: self.estimate,
            });
        }
        Ok(out)
    }
}

/// Budżet tła o stałym werdykcie.
pub struct FixedBudget {
    verdict: Mutex<BudgetVerdict>,
    checks: Mutex<Vec<Option<u64>>>,
    records: Mutex<Vec<LlmUsage>>,
}

impl FixedBudget {
    /// Zawsze pozwala.
    pub fn allow() -> Self {
        Self::with(BudgetVerdict::Allow)
    }

    /// Zawsze odmawia.
    pub fn deny() -> Self {
        Self::with(BudgetVerdict::Deny {
            reason: "limit tła wyczerpany".into(),
        })
    }

    fn with(verdict: BudgetVerdict) -> Self {
        Self {
            verdict: Mutex::new(verdict),
            checks: Mutex::new(Vec::new()),
            records: Mutex::new(Vec::new()),
        }
    }

    /// Zmienia werdykt.
    pub fn set(&self, verdict: BudgetVerdict) {
        *lock(&self.verdict) = verdict;
    }

    /// Sprawdzenia (szacunki).
    pub fn checks(&self) -> Vec<Option<u64>> {
        lock(&self.checks).clone()
    }

    /// Zarejestrowane zużycie.
    pub fn records(&self) -> Vec<LlmUsage> {
        lock(&self.records).clone()
    }
}

#[async_trait]
impl BackgroundBudget for FixedBudget {
    async fn check(&self, _model: &ConsolidatorModel, estimate: Option<u64>) -> BudgetVerdict {
        lock(&self.checks).push(estimate);
        lock(&self.verdict).clone()
    }

    async fn record(&self, usage: &LlmUsage) -> Result<(), ConsolidationError> {
        lock(&self.records).push(usage.clone());
        Ok(())
    }
}

/// Sterowany stan maszyny (domyślnie: sieć, bez pełnego ekranu, bezczynność 1 h, 03:00).
pub struct FakeHost {
    state: Mutex<HostState>,
}

impl Default for FakeHost {
    fn default() -> Self {
        Self {
            state: Mutex::new(HostState {
                on_battery: false,
                fullscreen: false,
                idle_secs: 3600,
                local_time: NaiveTime::from_hms_opt(3, 0, 0).unwrap_or_default(),
            }),
        }
    }
}

impl FakeHost {
    /// Stan nocny, bezczynny, na zasilaczu.
    pub fn idle_night() -> Self {
        Self::default()
    }

    /// Ustawia stan.
    pub fn set(&self, state: HostState) {
        *lock(&self.state) = state;
    }

    /// Zmienia stan funkcją.
    pub fn update(&self, f: impl FnOnce(&mut HostState)) {
        f(&mut lock(&self.state));
    }
}

impl HostConditions for FakeHost {
    fn state(&self) -> HostState {
        *lock(&self.state)
    }
}
