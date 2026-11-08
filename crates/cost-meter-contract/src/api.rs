//! Trait `CostMeter`, magazyn rekordów, zegar, błędy i nazwy zdarzeń.

use accounts_hub_contract::{ModelPrice, ProviderId};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use core_bus_contract::EventKind;

use crate::budget::{BudgetConfig, BudgetDecision, BudgetOrigin};
use crate::fx::{FxRate, RATE_SANITY_E4};
use crate::ledger::{Totals, TotalsQuery};
use crate::money::Usage;
use crate::record::{CostInput, CostRecord, Estimate};

/// Zdarzenie: zarejestrowano koszt.
pub const EVENT_COST_RECORDED: &str = "cost.recorded";
/// Zdarzenie: przekroczony próg alertu (np. 50/80/100%).
pub const EVENT_LIMIT_WARNING: &str = "cost.limit.warning";
/// Zdarzenie: `check_budget` zablokował zadanie.
pub const EVENT_LIMIT_BLOCKED: &str = "cost.limit.blocked";
/// Zdarzenie: pobrano nowy kurs NBP.
pub const EVENT_FX_UPDATED: &str = "cost.fx.updated";
/// Zdarzenie: brak kursu z dziś — użyto poprzedniego albo zapasowego.
pub const EVENT_FX_STALE: &str = "cost.fx.stale";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Błędy licznika kosztów.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CostError {
    /// Inicjator nie może zmieniać limitów (polityka Jądra).
    #[error("brak uprawnień do zmiany budżetu: {0:?}")]
    NotPermitted(BudgetOrigin),
    /// Niepoprawna konfiguracja.
    #[error("niepoprawna konfiguracja budżetu: {0}")]
    InvalidConfig(String),
    /// Błąd trwałego zapisu dziennika.
    #[error("błąd dziennika kosztów: {0}")]
    Storage(String),
}

/// Walidacja konfiguracji budżetu.
pub fn validate_budget(config: &BudgetConfig) -> Result<(), CostError> {
    let invalid = |m: &str| Err(CostError::InvalidConfig(m.to_owned()));
    if !RATE_SANITY_E4.contains(&config.fallback_rate_e4) {
        return invalid("kurs zapasowy poza zakresem 0,5–50 PLN/USD");
    }
    if config.warn_at_pct == 0 || config.warn_at_pct > 100 {
        return invalid("warn_at_pct musi być w zakresie 1–100");
    }
    if config.alert_thresholds_pct.contains(&0) {
        return invalid("progi alertów muszą być > 0");
    }
    Ok(())
}

/// Trwały dziennik rekordów (append-only).
pub trait LedgerStore: Send + Sync {
    /// Dopisuje rekord.
    fn append(&self, record: &CostRecord) -> Result<(), CostError>;

    /// Wczytuje wszystkie rekordy (uszkodzone linie są pomijane i liczone).
    fn load(&self) -> Result<LoadedLedger, CostError>;
}

/// Wynik odczytu dziennika.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadedLedger {
    /// Rekordy w kolejności zapisu.
    pub records: Vec<CostRecord>,
    /// Liczba pominiętych (uszkodzonych) linii.
    pub skipped_lines: usize,
}

/// Zegar: czas UTC i dzień lokalny (granice dni/miesięcy liczone lokalnie).
pub trait CostClock: Send + Sync {
    /// Teraz (UTC).
    fn now(&self) -> DateTime<Utc>;

    /// Dzisiejsza data lokalna.
    fn today(&self) -> NaiveDate;
}

/// Licznik kosztów i limitów (docs/modules/cost-meter/SPEC.md).
#[async_trait]
pub trait CostMeter: Send + Sync {
    /// Rejestruje koszt wywołania (przeliczenie na PLN bieżącym kursem); publikuje
    /// `cost.recorded` i ewentualnie `cost.limit.warning`.
    async fn record(&self, input: CostInput) -> Result<CostRecord, CostError>;

    /// Sumy liczone z rekordów.
    fn totals(&self, query: &TotalsQuery) -> Totals;

    /// Szacunek przed długim zadaniem (bieżący kurs).
    fn estimate(&self, usage: &Usage, price: &ModelPrice) -> Estimate;

    /// Decyzja Allow / Warn / Block dla zadania o szacowanym koszcie (mikro-PLN). Block tylko
    /// przy włączonym limicie; brak kursu nigdy nie blokuje. Publikuje `cost.limit.blocked`.
    async fn check_budget(
        &self,
        estimate_micro_pln: u64,
        background: bool,
        provider: Option<&ProviderId>,
    ) -> BudgetDecision;

    /// Bieżąca konfiguracja budżetu.
    fn budget(&self) -> BudgetConfig;

    /// Zmienia budżet (tylko użytkownik/Broker).
    async fn set_budget(&self, config: BudgetConfig, origin: BudgetOrigin)
    -> Result<(), CostError>;

    /// Kurs używany teraz.
    fn current_rate(&self) -> FxRate;

    /// Odświeża kurs, jeśli dziś jeszcze nie pobrano; przy błędzie — poprzedni albo zapasowy
    /// kurs i zdarzenie `cost.fx.stale`.
    async fn refresh_fx(&self) -> FxRate;
}
