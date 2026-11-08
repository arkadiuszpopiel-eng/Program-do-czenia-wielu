//! Polityka Diagnosty: autonomia napraw, limity częstotliwości i wychładzanie (PLAN §12.2).
//! Dana przez kompozycję (klucze `diagnostician.*` — Diagnosta ich nie zmienia).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::classify::ClassifierConfig;
use crate::journal::Consent;
use crate::plan::{Proposal, Risk};

/// Autonomia napraw (poza obszarem Jądra — tam zawsze Broker).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RepairAutonomy {
    /// Tylko propozycje — każdą naprawę zatwierdza użytkownik.
    ProposeOnly,
    /// Automatycznie naprawy o niskim ryzyku (domyślnie).
    AutoLowRisk,
    /// Automatycznie naprawy o ryzyku niskim i średnim.
    AutoMediumRisk,
}

/// Polityka.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RepairPolicy {
    /// Autonomia.
    pub autonomy: RepairAutonomy,
    /// Maksymalnie napraw automatycznych na godzinę.
    pub max_auto_repairs_per_hour: u32,
    /// Wychładzanie po nieudanej naprawie tego samego celu (ms).
    pub retry_cooldown_ms: u64,
    /// Po tylu nieudanych próbach — tylko człowiek.
    pub max_attempts: u32,
    /// Klasyfikator.
    pub classifier: ClassifierConfig,
    /// Maksymalna liczba sygnałów w pamięci.
    pub max_signals: usize,
    /// Pozycji w sekcjach raportu.
    pub report_items: usize,
}

impl Default for RepairPolicy {
    fn default() -> Self {
        Self {
            autonomy: RepairAutonomy::AutoLowRisk,
            max_auto_repairs_per_hour: 10,
            retry_cooldown_ms: 15 * 60 * 1000,
            max_attempts: 2,
            classifier: ClassifierConfig::default(),
            max_signals: 10_000,
            report_items: 20,
        }
    }
}

/// Kto zatwierdza propozycję.
pub fn consent_for(proposal: &Proposal, autonomy: RepairAutonomy) -> Consent {
    if proposal.kernel_area {
        return Consent::Broker;
    }
    if proposal.steps.is_empty() {
        return Consent::Human;
    }
    let auto = match autonomy {
        RepairAutonomy::ProposeOnly => false,
        RepairAutonomy::AutoLowRisk => proposal.risk == Risk::Low,
        RepairAutonomy::AutoMediumRisk => proposal.risk <= Risk::Medium,
    };
    if auto { Consent::Auto } else { Consent::User }
}
