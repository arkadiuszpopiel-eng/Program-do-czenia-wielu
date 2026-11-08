//! Agregaty kosztów liczone z rekordów (cache przyrostowy, odtwarzalny po restarcie).

use std::collections::BTreeMap;

use accounts_hub_contract::ProviderId;
use chrono::{Datelike, NaiveDate};
use core_bus_contract::SessionId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::record::CostRecord;

/// Miesiąc kalendarzowy (rok, miesiąc 1–12).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct Month {
    /// Rok.
    pub year: i32,
    /// Miesiąc 1–12.
    pub month: u32,
}

impl Month {
    /// Miesiąc dnia.
    pub fn of(day: NaiveDate) -> Self {
        Self {
            year: day.year(),
            month: day.month(),
        }
    }
}

/// Sumy (liczby całkowite; koszty nieznane liczone osobno).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Totals {
    /// Liczba wywołań.
    pub calls: u64,
    /// Wywołania bez znanej ceny.
    pub unknown_cost_calls: u64,
    /// Suma mikro-USD (tylko koszty znane).
    pub micro_usd: u64,
    /// Suma mikro-PLN (każdy rekord wg własnego kursu).
    pub micro_pln: u64,
    /// Tokeny wejściowe.
    pub input_tokens: u64,
    /// Tokeny wyjściowe.
    pub output_tokens: u64,
    /// Tokeny z cache.
    pub cache_read_tokens: u64,
    /// Tokeny zapisane do cache.
    pub cache_write_tokens: u64,
}

impl Totals {
    /// Dodaje rekord (nasycenie zamiast przepełnienia).
    pub fn add(&mut self, r: &CostRecord) {
        self.calls = self.calls.saturating_add(1);
        match (r.micro_usd, r.micro_pln) {
            (Some(usd), Some(pln)) => {
                self.micro_usd = self.micro_usd.saturating_add(usd);
                self.micro_pln = self.micro_pln.saturating_add(pln);
            }
            _ => self.unknown_cost_calls = self.unknown_cost_calls.saturating_add(1),
        }
        self.input_tokens = self.input_tokens.saturating_add(r.usage.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(r.usage.output_tokens);
        self.cache_read_tokens = self
            .cache_read_tokens
            .saturating_add(r.usage.cache_read_tokens);
        self.cache_write_tokens = self
            .cache_write_tokens
            .saturating_add(r.usage.cache_write_tokens);
    }
}

/// Zapytanie o sumy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum TotalsQuery {
    /// Wszystko.
    All,
    /// Sesja.
    Session {
        /// Sesja.
        session: SessionId,
    },
    /// Dzień lokalny.
    Day {
        /// Dzień.
        day: NaiveDate,
    },
    /// Miesiąc.
    Month {
        /// Miesiąc.
        month: Month,
    },
    /// Dostawca w miesiącu.
    Provider {
        /// Dostawca.
        provider: ProviderId,
        /// Miesiąc.
        month: Month,
    },
    /// Zadania tła w miesiącu.
    Background {
        /// Miesiąc.
        month: Month,
    },
}

/// Dziennik w pamięci z agregatami przyrostowymi.
#[derive(Debug, Clone, Default)]
pub struct Ledger {
    records: Vec<CostRecord>,
    all: Totals,
    by_session: BTreeMap<SessionId, Totals>,
    by_day: BTreeMap<NaiveDate, Totals>,
    by_month: BTreeMap<Month, Totals>,
    by_provider: BTreeMap<(ProviderId, Month), Totals>,
    background: BTreeMap<Month, Totals>,
}

impl Ledger {
    /// Dziennik z rekordów (np. po odczycie NDJSON).
    pub fn from_records<I: IntoIterator<Item = CostRecord>>(records: I) -> Self {
        let mut ledger = Self::default();
        for r in records {
            ledger.push(r);
        }
        ledger
    }

    /// Dodaje rekord i aktualizuje agregaty.
    pub fn push(&mut self, r: CostRecord) {
        let month = Month::of(r.day);
        self.all.add(&r);
        if let Some(s) = &r.session {
            self.by_session.entry(s.clone()).or_default().add(&r);
        }
        self.by_day.entry(r.day).or_default().add(&r);
        self.by_month.entry(month).or_default().add(&r);
        self.by_provider
            .entry((r.provider.clone(), month))
            .or_default()
            .add(&r);
        if r.background {
            self.background.entry(month).or_default().add(&r);
        }
        self.records.push(r);
    }

    /// Sumy dla zapytania.
    pub fn totals(&self, q: &TotalsQuery) -> Totals {
        let found = match q {
            TotalsQuery::All => Some(&self.all),
            TotalsQuery::Session { session } => self.by_session.get(session),
            TotalsQuery::Day { day } => self.by_day.get(day),
            TotalsQuery::Month { month } => self.by_month.get(month),
            TotalsQuery::Provider { provider, month } => {
                self.by_provider.get(&(provider.clone(), *month))
            }
            TotalsQuery::Background { month } => self.background.get(month),
        };
        found.copied().unwrap_or_default()
    }

    /// Wszystkie rekordy w kolejności rejestracji.
    pub fn records(&self) -> &[CostRecord] {
        &self.records
    }

    /// Ostatni numer kolejny (0, gdy pusty).
    pub fn last_seq(&self) -> u64 {
        self.records.last().map_or(0, |r| r.seq)
    }

    /// Sesje z kosztami.
    pub fn sessions(&self) -> Vec<SessionId> {
        self.by_session.keys().cloned().collect()
    }

    /// Dni z kosztami.
    pub fn days(&self) -> Vec<NaiveDate> {
        self.by_day.keys().copied().collect()
    }

    /// Miesiące z kosztami.
    pub fn months(&self) -> Vec<Month> {
        self.by_month.keys().copied().collect()
    }

    /// Pary (dostawca, miesiąc) z kosztami.
    pub fn provider_months(&self) -> Vec<(ProviderId, Month)> {
        self.by_provider.keys().cloned().collect()
    }
}
