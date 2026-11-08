//! `CostMeterService`: rejestracja kosztów, agregaty, kurs, limity i alerty.

use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use accounts_hub_contract::{ModelPrice, ProviderId};
use async_trait::async_trait;
use chrono::{DateTime, Local, NaiveDate, Utc};
use core_bus_contract::{Event, EventBus, Level};
use core_registry_contract::{ManifestError, ModuleManifest};
use cost_meter_contract::{
    BudgetConfig, BudgetDecision, BudgetOrigin, BudgetScope, CostClock, CostError, CostInput,
    CostMeter, CostRecord, EVENT_COST_RECORDED, EVENT_FX_STALE, EVENT_FX_UPDATED,
    EVENT_LIMIT_BLOCKED, EVENT_LIMIT_WARNING, Estimate, FxCache, FxOrigin, FxQuote, FxRate,
    FxSource, Ledger, LedgerStore, LimitMode, Month, MonthlyLimit, Spent, Totals, TotalsQuery,
    Usage, crossed_thresholds, estimate, evaluate, event_kind, validate_budget,
};

use crate::MODULE_TOML;

/// Zegar systemowy: UTC + data lokalna.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl CostClock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn today(&self) -> NaiveDate {
        Local::now().date_naive()
    }
}

struct State {
    ledger: Ledger,
    fx: FxCache,
    budget: BudgetConfig,
}

/// Licznik kosztów.
pub struct CostMeterService {
    pub(crate) manifest: ModuleManifest,
    clock: Arc<dyn CostClock>,
    fx_source: Arc<dyn FxSource>,
    store: Arc<dyn LedgerStore>,
    state: Mutex<State>,
    pub(crate) bus: RwLock<Option<Arc<dyn EventBus>>>,
    skipped_lines: usize,
}

type Alert = (BudgetScope, u8, u64, u64);

impl CostMeterService {
    /// Buduje licznik: odtwarza agregaty z dziennika i kurs z ostatniego rekordu NBP.
    pub fn new(
        store: Arc<dyn LedgerStore>,
        fx_source: Arc<dyn FxSource>,
        clock: Arc<dyn CostClock>,
        budget: BudgetConfig,
    ) -> Result<Self, CostError> {
        let manifest = ModuleManifest::parse_toml(MODULE_TOML)
            .map_err(|e: ManifestError| CostError::InvalidConfig(e.to_string()))?;
        validate_budget(&budget)?;
        let loaded = store.load()?;
        let mut fx = FxCache::new(budget.fallback_rate_e4);
        let last_nbp = loaded
            .records
            .iter()
            .rev()
            .find(|r| r.fx.origin == FxOrigin::Nbp);
        if let Some(r) = last_nbp
            && let Some(effective_date) = r.fx.effective_date
        {
            let quote = FxQuote {
                rate_e4: r.fx.rate_e4,
                effective_date,
            };
            fx.store(quote, r.day);
        }
        Ok(Self {
            manifest,
            clock,
            fx_source,
            store,
            state: Mutex::new(State {
                ledger: Ledger::from_records(loaded.records),
                fx,
                budget,
            }),
            bus: RwLock::new(None),
            skipped_lines: loaded.skipped_lines,
        })
    }

    /// Liczba uszkodzonych linii pominiętych przy odczycie dziennika.
    pub fn skipped_lines(&self) -> usize {
        self.skipped_lines
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    async fn publish(&self, name: &str, level: Level, payload: serde_json::Value) {
        let bus = self.bus.read().unwrap_or_else(|p| p.into_inner()).clone();
        if let Some(bus) = bus {
            // Zdarzenia są informacyjne; błąd magistrali nie cofa rejestracji.
            let _ = bus
                .publish(Event::new(event_kind(name), level, payload))
                .await;
        }
    }
}

fn spent(ledger: &Ledger, month: Month, provider: Option<&ProviderId>) -> Spent {
    Spent {
        month_micro_pln: ledger.totals(&TotalsQuery::Month { month }).micro_pln,
        background_micro_pln: ledger.totals(&TotalsQuery::Background { month }).micro_pln,
        provider_micro_pln: provider.map_or(0, |p| {
            ledger
                .totals(&TotalsQuery::Provider {
                    provider: p.clone(),
                    month,
                })
                .micro_pln
        }),
    }
}

/// Progi przekroczone przez rekord w każdym zakresie z włączonymi alertami.
fn alerts(budget: &BudgetConfig, before: Spent, after: Spent, r: &CostRecord) -> Vec<Alert> {
    let mut scopes: Vec<(BudgetScope, MonthlyLimit, u64, u64)> = vec![(
        BudgetScope::Monthly,
        budget.monthly,
        before.month_micro_pln,
        after.month_micro_pln,
    )];
    if r.background {
        scopes.push((
            BudgetScope::Background,
            budget.background,
            before.background_micro_pln,
            after.background_micro_pln,
        ));
    }
    if let Some(limit) = budget.providers.get(&r.provider) {
        scopes.push((
            BudgetScope::Provider(r.provider.clone()),
            MonthlyLimit::from(*limit),
            before.provider_micro_pln,
            after.provider_micro_pln,
        ));
    }
    scopes
        .into_iter()
        .filter(|(_, limit, _, _)| limit.mode != LimitMode::Off)
        .flat_map(|(scope, limit, b, a)| {
            crossed_thresholds(b, a, limit.amount_micro_pln, &budget.alert_thresholds_pct)
                .into_iter()
                .map(move |t| (scope.clone(), t, a, limit.amount_micro_pln))
        })
        .collect()
}

#[async_trait]
impl CostMeter for CostMeterService {
    async fn record(&self, input: CostInput) -> Result<CostRecord, CostError> {
        let (record, fired) = {
            let mut st = self.lock();
            let today = self.clock.today();
            let fx = st.fx.current(today);
            let seq = st.ledger.last_seq() + 1;
            let record = CostRecord::from_input(input, seq, self.clock.now(), today, fx);
            let month = Month::of(record.day);
            let before = spent(&st.ledger, month, Some(&record.provider));
            self.store.append(&record)?;
            st.ledger.push(record.clone());
            let after = spent(&st.ledger, month, Some(&record.provider));
            let fired = alerts(&st.budget, before, after, &record);
            (record, fired)
        };
        let payload = serde_json::json!({
            "seq": record.seq, "provider": record.provider, "model": record.model,
            "session": record.session, "micro_usd": record.micro_usd, "micro_pln": record.micro_pln,
            "background": record.background, "fx_origin": record.fx.origin,
        });
        self.publish(EVENT_COST_RECORDED, Level::Debug, payload)
            .await;
        for (scope, threshold, spent, limit) in fired {
            let payload = serde_json::json!({
                "scope": scope, "threshold_pct": threshold, "spent_micro_pln": spent,
                "limit_micro_pln": limit,
            });
            self.publish(EVENT_LIMIT_WARNING, Level::Warn, payload)
                .await;
        }
        Ok(record)
    }

    fn totals(&self, query: &TotalsQuery) -> Totals {
        self.lock().ledger.totals(query)
    }

    fn estimate(&self, usage: &Usage, price: &ModelPrice) -> Estimate {
        let rate = self.current_rate();
        estimate(usage, price, rate)
    }

    async fn check_budget(
        &self,
        estimate_micro_pln: u64,
        background: bool,
        provider: Option<&ProviderId>,
    ) -> BudgetDecision {
        let decision = {
            let st = self.lock();
            let month = Month::of(self.clock.today());
            let spent = spent(&st.ledger, month, provider);
            evaluate(&st.budget, spent, estimate_micro_pln, background, provider)
        };
        if let BudgetDecision::Block { notice } = &decision {
            let payload = serde_json::to_value(notice).unwrap_or_default();
            self.publish(EVENT_LIMIT_BLOCKED, Level::Warn, payload)
                .await;
        }
        decision
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
        let mut st = self.lock();
        st.fx.set_fallback(config.fallback_rate_e4);
        st.budget = config;
        Ok(())
    }

    fn current_rate(&self) -> FxRate {
        self.lock().fx.current(self.clock.today())
    }

    async fn refresh_fx(&self) -> FxRate {
        let today = self.clock.today();
        if !self.lock().fx.needs_refresh(today) {
            return self.current_rate();
        }
        match self.fx_source.fetch_usd_pln().await {
            Ok(quote) => {
                self.lock().fx.store(quote, today);
                let payload = serde_json::json!({
                    "rate_e4": quote.rate_e4, "effective_date": quote.effective_date,
                });
                self.publish(EVENT_FX_UPDATED, Level::Info, payload).await;
            }
            Err(e) => {
                let rate = self.current_rate();
                let payload = serde_json::json!({
                    "reason": e.to_string(), "rate_e4": rate.rate_e4, "origin": rate.origin,
                });
                self.publish(EVENT_FX_STALE, Level::Warn, payload).await;
            }
        }
        self.current_rate()
    }
}
