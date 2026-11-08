//! Wejście rejestracji kosztu, rekord kosztu i szacunek.

use accounts_hub_contract::{AccountId, ModelId, ModelPrice, ProviderId};
use chrono::{DateTime, NaiveDate, Utc};
use core_bus_contract::{AgentId, SessionId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fx::FxRate;
use crate::money::{Usage, cost_micro_usd, usd_to_pln};

/// Skąd znamy koszt wywołania.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "pricing", rename_all = "snake_case")]
pub enum Pricing {
    /// Cena z cennika w konfiguracji.
    Price(ModelPrice),
    /// Koszt podany przez dostawcę (mikro-USD).
    Reported {
        /// Koszt w mikro-USD.
        micro_usd: u64,
    },
    /// Brak ceny — koszt „nieznany” (w UI oznaczony, nigdy liczony jako 0).
    Unknown,
}

/// Dane wywołania do zarejestrowania.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CostInput {
    /// Sesja.
    pub session: Option<SessionId>,
    /// Agentka.
    pub agent: Option<AgentId>,
    /// Dostawca.
    pub provider: ProviderId,
    /// Konto.
    pub account: Option<AccountId>,
    /// Model.
    pub model: ModelId,
    /// Zużycie.
    pub usage: Usage,
    /// Cena.
    pub pricing: Pricing,
    /// Zadanie tła (osobny budżet).
    pub background: bool,
}

/// Zarejestrowany koszt (niezmienny; linia NDJSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CostRecord {
    /// Numer kolejny (od 1, monotoniczny w dzienniku).
    pub seq: u64,
    /// Czas rejestracji (UTC).
    pub ts: DateTime<Utc>,
    /// Dzień lokalny, do którego rekord należy (agregaty dzienne/miesięczne).
    pub day: NaiveDate,
    /// Sesja.
    pub session: Option<SessionId>,
    /// Agentka.
    pub agent: Option<AgentId>,
    /// Dostawca.
    pub provider: ProviderId,
    /// Konto.
    pub account: Option<AccountId>,
    /// Model.
    pub model: ModelId,
    /// Zużycie.
    pub usage: Usage,
    /// Koszt w mikro-USD; `None` = nieznany.
    pub micro_usd: Option<u64>,
    /// Koszt w mikro-PLN wg kursu z rekordu; `None` = nieznany.
    pub micro_pln: Option<u64>,
    /// Kurs użyty do przeliczenia.
    pub fx: FxRate,
    /// Zadanie tła.
    pub background: bool,
}

impl CostRecord {
    /// Buduje rekord z wejścia (koszt liczony z ceny albo przejęty od dostawcy).
    pub fn from_input(
        input: CostInput,
        seq: u64,
        ts: DateTime<Utc>,
        day: NaiveDate,
        fx: FxRate,
    ) -> Self {
        let micro_usd = match input.pricing {
            Pricing::Price(price) => Some(cost_micro_usd(&input.usage, &price)),
            Pricing::Reported { micro_usd } => Some(micro_usd),
            Pricing::Unknown => None,
        };
        Self {
            seq,
            ts,
            day,
            session: input.session,
            agent: input.agent,
            provider: input.provider,
            account: input.account,
            model: input.model,
            usage: input.usage,
            micro_usd,
            micro_pln: micro_usd.map(|usd| usd_to_pln(usd, fx.rate_e4)),
            fx,
            background: input.background,
        }
    }
}

/// Szacunek kosztu przed zadaniem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Estimate {
    /// Mikro-USD.
    pub micro_usd: u64,
    /// Mikro-PLN wg bieżącego kursu.
    pub micro_pln: u64,
    /// Kurs.
    pub fx: FxRate,
}

/// Szacunek: tokeny × cena, przeliczone bieżącym kursem.
pub fn estimate(usage: &Usage, price: &ModelPrice, fx: FxRate) -> Estimate {
    let micro_usd = cost_micro_usd(usage, price);
    Estimate {
        micro_usd,
        micro_pln: usd_to_pln(micro_usd, fx.rate_e4),
        fx,
    }
}
