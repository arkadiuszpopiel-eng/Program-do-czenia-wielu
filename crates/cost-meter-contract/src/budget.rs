//! Limity i decyzja budżetowa: miesięczny limit PLN (wyłączalny), budżet tła, limity dostawców,
//! progi alertów. Czysta logika współdzielona przez `-impl` i `-fake`.

use std::collections::BTreeMap;

use accounts_hub_contract::{CostLimit, ProviderId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fx::DEFAULT_FALLBACK_RATE_E4;
use crate::money::{grosze_to_micro_pln, percent};

/// Tryb limitu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LimitMode {
    /// Limit włączony: ostrzeżenia i blokada po przekroczeniu.
    Enforced,
    /// Limit wyłączony, ale z alertami (wskaźnik + progi, nigdy blokada).
    AlertOnly,
    /// Całkowicie wyłączony: tylko wskaźnik zużycia.
    Off,
}

/// Limit miesięczny w mikro-PLN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MonthlyLimit {
    /// Kwota w mikro-PLN (1 PLN = 1 000 000).
    pub amount_micro_pln: u64,
    /// Tryb.
    pub mode: LimitMode,
}

impl MonthlyLimit {
    /// Limit w pełnych złotych.
    pub fn pln(pln: u64, mode: LimitMode) -> Self {
        Self {
            amount_micro_pln: grosze_to_micro_pln(pln.saturating_mul(100)),
            mode,
        }
    }
}

impl From<CostLimit> for MonthlyLimit {
    fn from(l: CostLimit) -> Self {
        Self {
            amount_micro_pln: grosze_to_micro_pln(l.monthly_limit_grosze),
            mode: if l.enabled {
                LimitMode::Enforced
            } else {
                LimitMode::AlertOnly
            },
        }
    }
}

/// Konfiguracja budżetu (`[cost]` w TOML; polityka Jądra — Ulepszacz jej nie zmienia).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BudgetConfig {
    /// Limit miesięczny całości.
    pub monthly: MonthlyLimit,
    /// Budżet zadań tła (domyślnie 0 PLN = tylko modele lokalne).
    pub background: MonthlyLimit,
    /// Limity per dostawca (z `accounts-hub`).
    pub providers: BTreeMap<ProviderId, CostLimit>,
    /// Próg ostrzeżenia w `check_budget` (procent limitu po wykonaniu zadania).
    pub warn_at_pct: u8,
    /// Progi alertów przy rejestracji (np. 50/80/100).
    pub alert_thresholds_pct: Vec<u8>,
    /// Kurs zapasowy ×10⁴ (gdy brak kursu NBP).
    pub fallback_rate_e4: u32,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            monthly: MonthlyLimit::pln(100, LimitMode::Enforced),
            background: MonthlyLimit::pln(0, LimitMode::Enforced),
            providers: BTreeMap::new(),
            warn_at_pct: 80,
            alert_thresholds_pct: vec![50, 80, 100],
            fallback_rate_e4: DEFAULT_FALLBACK_RATE_E4,
        }
    }
}

/// Zakres limitu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "scope", content = "provider", rename_all = "snake_case")]
pub enum BudgetScope {
    /// Limit miesięczny całości.
    Monthly,
    /// Budżet tła.
    Background,
    /// Limit dostawcy.
    Provider(ProviderId),
}

/// Szczegóły ostrzeżenia/blokady.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BudgetNotice {
    /// Zakres.
    pub scope: BudgetScope,
    /// Wydano w tym miesiącu (mikro-PLN).
    pub spent_micro_pln: u64,
    /// Szacunek zadania (mikro-PLN).
    pub estimate_micro_pln: u64,
    /// Limit (mikro-PLN).
    pub limit_micro_pln: u64,
    /// Procent limitu po wykonaniu zadania.
    pub pct_after: u32,
}

/// Decyzja `check_budget`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum BudgetDecision {
    /// Można.
    Allow,
    /// Można, ale z ostrzeżeniem.
    Warn {
        /// Powody.
        notices: Vec<BudgetNotice>,
    },
    /// Nie można (tylko gdy odpowiedni limit jest włączony).
    Block {
        /// Powód.
        notice: BudgetNotice,
    },
}

/// Wydatki w bieżącym miesiącu potrzebne do decyzji.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Spent {
    /// Całość.
    pub month_micro_pln: u64,
    /// Zadania tła.
    pub background_micro_pln: u64,
    /// Dostawca zadania (jeśli podany).
    pub provider_micro_pln: u64,
}

/// Decyzja budżetowa. Kolejność sprawdzania: miesiąc → tło → dostawca; pierwsza blokada wygrywa.
pub fn evaluate(
    config: &BudgetConfig,
    spent: Spent,
    estimate_micro_pln: u64,
    background: bool,
    provider: Option<&ProviderId>,
) -> BudgetDecision {
    let mut scopes = vec![(BudgetScope::Monthly, config.monthly, spent.month_micro_pln)];
    if background {
        scopes.push((
            BudgetScope::Background,
            config.background,
            spent.background_micro_pln,
        ));
    }
    if let Some(p) = provider
        && let Some(limit) = config.providers.get(p)
    {
        scopes.push((
            BudgetScope::Provider(p.clone()),
            MonthlyLimit::from(*limit),
            spent.provider_micro_pln,
        ));
    }
    let mut notices = Vec::new();
    for (scope, limit, spent) in scopes {
        if limit.mode == LimitMode::Off {
            continue;
        }
        let after = spent.saturating_add(estimate_micro_pln);
        let notice = BudgetNotice {
            scope,
            spent_micro_pln: spent,
            estimate_micro_pln,
            limit_micro_pln: limit.amount_micro_pln,
            pct_after: percent(after, limit.amount_micro_pln),
        };
        if limit.mode == LimitMode::Enforced && after > limit.amount_micro_pln {
            return BudgetDecision::Block { notice };
        }
        if notice.pct_after >= u32::from(config.warn_at_pct) {
            notices.push(notice);
        }
    }
    if notices.is_empty() {
        BudgetDecision::Allow
    } else {
        BudgetDecision::Warn { notices }
    }
}

/// Progi przekroczone przez przejście `before → after` (każdy próg raz: `before < t ≤ after`).
pub fn crossed_thresholds(before: u64, after: u64, limit: u64, thresholds: &[u8]) -> Vec<u8> {
    let (b, a) = (percent(before, limit), percent(after, limit));
    let mut crossed: Vec<u8> = thresholds
        .iter()
        .copied()
        .filter(|t| b < u32::from(*t) && u32::from(*t) <= a)
        .collect();
    crossed.sort_unstable();
    crossed.dedup();
    crossed
}

/// Kto zmienia budżet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "origin", content = "id", rename_all = "snake_case")]
pub enum BudgetOrigin {
    /// Użytkownik (ustawienia).
    User,
    /// Broker.
    Broker,
    /// Ulepszacz — nie zmienia limitów (polityka Jądra).
    Improver,
    /// Agentka — nie zmienia limitów.
    Agent(String),
}

impl BudgetOrigin {
    /// Limity zmienia tylko użytkownik albo Broker.
    pub fn may_change_budget(&self) -> bool {
        matches!(self, BudgetOrigin::User | BudgetOrigin::Broker)
    }
}
