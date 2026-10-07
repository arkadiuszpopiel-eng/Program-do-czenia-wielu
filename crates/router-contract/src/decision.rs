//! Decyzja trasy z uzasadnieniem, powody odrzucenia, ostrzeżenia, wynik wywołania.

use std::fmt;

use compliance_contract::DecisionReason;
use cost_meter_contract::BudgetNotice;
use providers_contract::{ProviderError, ProviderErrorKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::TaskClass;
use crate::candidate::Candidate;
use crate::needs::MissingCapability;

/// Dlaczego kandydat odpadł (stabilny kod dla UI: „brakuje: klucz X").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum RejectReason {
    /// Dostawca nie jest zarejestrowany w Routerze.
    NotRegistered,
    /// Brak klucza — trasa niewidoczna (PLAN §5.6).
    Unconfigured,
    /// Klucz odrzucony (401/403) — wymaga działania użytkownika.
    AuthFailed,
    /// Rejestr zgodności: trasa zabroniona/wyłączona/niedozwolona dla sesji.
    Compliance {
        /// Powód z `route_allowed`.
        reason: DecisionReason,
    },
    /// Polityka prywatności dostawcy (obrona w głąb, `check_privacy`).
    Privacy {
        /// Opis.
        message: String,
    },
    /// Jurysdykcja dostawcy spoza dozwolonych dla sesji.
    Jurisdiction {
        /// Jurysdykcja dostawcy (`unknown` = nieznana).
        jurisdiction: String,
    },
    /// Model nie ma wymaganej możliwości.
    Capability {
        /// Czego brakuje.
        missing: MissingCapability,
    },
    /// Limit kosztów (`cost-meter` → `Block`).
    Budget {
        /// Szczegóły limitu.
        notice: BudgetNotice,
    },
    /// Obwód otwarty po serii błędów.
    CircuitOpen {
        /// Za ile ms próba (half-open).
        retry_in_ms: u64,
    },
    /// Okno limitu wyczerpane (429) — estymata reaktywna.
    PlanWindow {
        /// Za ile ms okno się odnowi (estymata).
        retry_in_ms: u64,
    },
    /// Ostatni czas do pierwszego tokenu powyżej limitu zadania.
    Latency {
        /// Zmierzony TTFT (ms).
        ttft_ms: u64,
        /// Limit (ms).
        max_ms: u64,
    },
}

impl fmt::Display for RejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRegistered => f.write_str("dostawca niezarejestrowany"),
            Self::Unconfigured => f.write_str("brak klucza API (dodaj klucz w Ustawieniach)"),
            Self::AuthFailed => f.write_str("klucz odrzucony przez dostawcę"),
            Self::Compliance { reason } => write!(f, "zgodność: {reason}"),
            Self::Privacy { message } => write!(f, "prywatność: {message}"),
            Self::Jurisdiction { jurisdiction } => {
                write!(f, "jurysdykcja {jurisdiction} niedozwolona w tej sesji")
            }
            Self::Capability { missing } => write!(f, "model nie obsługuje: {missing:?}"),
            Self::Budget { notice } => write!(
                f,
                "limit kosztów ({:?}) — {} % po zadaniu",
                notice.scope, notice.pct_after
            ),
            Self::CircuitOpen { retry_in_ms } => {
                write!(f, "obwód otwarty po błędach (próba za {retry_in_ms} ms)")
            }
            Self::PlanWindow { retry_in_ms } => {
                write!(
                    f,
                    "limit zapytań wyczerpany (odnowienie za ~{retry_in_ms} ms)"
                )
            }
            Self::Latency { ttft_ms, max_ms } => {
                write!(
                    f,
                    "za wolny: {ttft_ms} ms > {max_ms} ms do pierwszego tokenu"
                )
            }
        }
    }
}

/// Ostrzeżenie przy dozwolonej trasie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum RouteWarning {
    /// Trasa szara / niezweryfikowana w rejestrze zgodności.
    Compliance {
        /// Powód.
        reason: DecisionReason,
    },
    /// Blisko limitu kosztów.
    Budget {
        /// Progi.
        notices: Vec<BudgetNotice>,
    },
    /// Brak cennika modelu — koszt nieznany, limit nieegzekwowalny.
    NoPricing,
    /// Model nieznany dostawcy (brak w możliwościach) — wymagania niesprawdzone.
    UnknownCapabilities,
}

/// Decyzja Routera (bez treści rozmowy — publikowana jako `router.decision`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteDecision {
    /// Klasa zadania.
    pub class: TaskClass,
    /// Wybrany cel.
    pub chosen: Candidate,
    /// Kolejne dozwolone cele (fallback w tej kolejności).
    pub fallbacks: Vec<Candidate>,
    /// Odrzuceni kandydaci z powodem.
    pub rejected: Vec<(Candidate, RejectReason)>,
    /// Ostrzeżenia dozwolonych kandydatów.
    pub warnings: Vec<(Candidate, RouteWarning)>,
}

impl RouteDecision {
    /// Cele w kolejności prób: wybrany, potem fallbacki.
    pub fn targets(&self) -> impl Iterator<Item = &Candidate> {
        std::iter::once(&self.chosen).chain(self.fallbacks.iter())
    }
}

/// Błąd wyboru trasy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum RouteError {
    /// Żaden kandydat nie spełnia ograniczeń (`router.no_route` — UI: czego brakuje).
    #[error("brak trasy dla klasy {class:?}: {}", describe(rejected))]
    NoRoute {
        /// Klasa.
        class: TaskClass,
        /// Odrzuceni kandydaci z powodem (pusta lista = brak kandydatów w polityce).
        rejected: Vec<(Candidate, RejectReason)>,
    },
}

fn describe(rejected: &[(Candidate, RejectReason)]) -> String {
    if rejected.is_empty() {
        return "brak kandydatów w polityce".into();
    }
    rejected
        .iter()
        .map(|(c, r)| format!("{c} — {r}"))
        .collect::<Vec<_>>()
        .join("; ")
}

impl RouteError {
    /// Błąd dostawcy dla strumienia (Router jako `ModelProvider`): same odmowy prywatności
    /// i zgodności → `PrivacyBlocked`, inaczej `Unsupported`.
    pub fn to_provider_error(&self) -> ProviderError {
        let Self::NoRoute { rejected, .. } = self;
        let privacy = !rejected.is_empty()
            && rejected.iter().all(|(_, r)| {
                matches!(
                    r,
                    RejectReason::Compliance { .. }
                        | RejectReason::Privacy { .. }
                        | RejectReason::Jurisdiction { .. }
                )
            });
        let kind = if privacy {
            ProviderErrorKind::PrivacyBlocked
        } else {
            ProviderErrorKind::Unsupported
        };
        ProviderError::new(kind, self.to_string())
    }
}

/// Wynik wywołania zgłaszany Routerowi (`report`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// Sukces.
    Ok {
        /// Czas do pierwszego tokenu (ms).
        ttft_ms: Option<u64>,
        /// Czas całkowity (ms).
        latency_ms: u64,
    },
    /// Błąd dostawcy (także przekroczony termin pierwszego zdarzenia Routera).
    Failed {
        /// Rodzaj.
        kind: ProviderErrorKind,
    },
    /// Anulowane przez użytkownika — neutralne dla obwodu.
    Cancelled,
}
