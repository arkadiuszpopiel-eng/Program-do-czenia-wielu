//! `FakeCostMeter`: rekordy w pamięci, stały kurs, sterowany werdykt `check_budget`.

use std::sync::{Mutex, MutexGuard};

use accounts_hub_contract::{ModelPrice, ProviderId};
use async_trait::async_trait;
use chrono::{NaiveDate, TimeZone, Utc};
use cost_meter_contract::{
    BudgetConfig, BudgetDecision, BudgetOrigin, CostError, CostInput, CostMeter, CostRecord,
    Estimate, FxOrigin, FxRate, Ledger, Month, Spent, Totals, TotalsQuery, Usage, estimate,
    evaluate, validate_budget,
};

/// Zapytanie `check_budget` zarejestrowane przez atrapę.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetCheck {
    /// Szacunek (mikro-PLN).
    pub estimate_micro_pln: u64,
    /// Zadanie tła.
    pub background: bool,
    /// Dostawca.
    pub provider: Option<ProviderId>,
}

struct State {
    ledger: Ledger,
    budget: BudgetConfig,
    rate: FxRate,
    day: NaiveDate,
    forced: Option<BudgetDecision>,
    checks: Vec<BudgetCheck>,
}

/// Atrapa `CostMeter` dla testów `agent-runtime`/`router`.
pub struct FakeCostMeter {
    state: Mutex<State>,
}

impl FakeCostMeter {
    /// Atrapa ze stałym kursem NBP (×10⁴) i dniem.
    pub fn new(rate_e4: u32, day: NaiveDate) -> Self {
        Self {
            state: Mutex::new(State {
                ledger: Ledger::default(),
                budget: BudgetConfig::default(),
                rate: FxRate {
                    rate_e4,
                    effective_date: Some(day),
                    origin: FxOrigin::Nbp,
                    stale: false,
                },
                day,
                forced: None,
                checks: Vec::new(),
            }),
        }
    }

    /// Wymusza werdykt `check_budget` (`None` = liczony z limitów).
    pub fn force_decision(&self, decision: Option<BudgetDecision>) {
        self.lock().forced = decision;
    }

    /// Zapytania `check_budget`.
    pub fn checks(&self) -> Vec<BudgetCheck> {
        self.lock().checks.clone()
    }

    /// Zarejestrowane rekordy.
    pub fn records(&self) -> Vec<CostRecord> {
        self.lock().ledger.records().to_vec()
    }

    /// Ustawia dzień (granice dni/miesięcy).
    pub fn set_day(&self, day: NaiveDate) {
        self.lock().day = day;
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[async_trait]
impl CostMeter for FakeCostMeter {
    async fn record(&self, input: CostInput) -> Result<CostRecord, CostError> {
        let mut st = self.lock();
        let seq = st.ledger.last_seq() + 1;
        let ts = st
            .day
            .and_hms_opt(12, 0, 0)
            .map(|dt| Utc.from_utc_datetime(&dt))
            .unwrap_or_default();
        let record = CostRecord::from_input(input, seq, ts, st.day, st.rate);
        st.ledger.push(record.clone());
        Ok(record)
    }

    fn totals(&self, query: &TotalsQuery) -> Totals {
        self.lock().ledger.totals(query)
    }

    fn estimate(&self, usage: &Usage, price: &ModelPrice) -> Estimate {
        estimate(usage, price, self.lock().rate)
    }

    async fn check_budget(
        &self,
        estimate_micro_pln: u64,
        background: bool,
        provider: Option<&ProviderId>,
    ) -> BudgetDecision {
        let mut st = self.lock();
        st.checks.push(BudgetCheck {
            estimate_micro_pln,
            background,
            provider: provider.cloned(),
        });
        if let Some(forced) = &st.forced {
            return forced.clone();
        }
        let month = Month::of(st.day);
        let spent = Spent {
            month_micro_pln: st.ledger.totals(&TotalsQuery::Month { month }).micro_pln,
            background_micro_pln: st
                .ledger
                .totals(&TotalsQuery::Background { month })
                .micro_pln,
            provider_micro_pln: provider.map_or(0, |p| {
                st.ledger
                    .totals(&TotalsQuery::Provider {
                        provider: p.clone(),
                        month,
                    })
                    .micro_pln
            }),
        };
        evaluate(&st.budget, spent, estimate_micro_pln, background, provider)
    }

    fn budget(&self) -> BudgetConfig {
        self.lock().budget.clone()
    }

    async fn set_budget(
        &self,
        config: BudgetConfig,
        origin: BudgetOrigin,
    ) -> Result<(), CostError> {
        if !origin.may_change_budget() {
            return Err(CostError::NotPermitted(origin));
        }
        validate_budget(&config)?;
        self.lock().budget = config;
        Ok(())
    }

    fn current_rate(&self) -> FxRate {
        self.lock().rate
    }

    async fn refresh_fx(&self) -> FxRate {
        self.lock().rate
    }
}
