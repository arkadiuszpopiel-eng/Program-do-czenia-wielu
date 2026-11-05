//! Powód odmowy wykonania akcji przez narzędzie (dla modelu i UI).

use risk_classifier_contract::KernelRule;
use safety_broker_contract::ApprovalId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Dlaczego narzędzie nie wykonało akcji.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum DenialReason {
    /// Twarda blokada Jądra (każdy poziom, także L4).
    KernelBlock {
        /// Reguła.
        rule: KernelRule,
    },
    /// Ścieżka/polecenie na deny-liście (sprawdzenie narzędzia przed Brokerem).
    DenyList,
    /// Właściciel odmówił w Broker-UI.
    OwnerDenied {
        /// Prośba.
        approval: ApprovalId,
    },
    /// Model-sędzia trybu „Automatycznie” odmówił (plan 50 §4.3).
    JudgeDenied {
        /// Prośba.
        approval: ApprovalId,
        /// Kategoria i powód po polsku (`reason` zajmuje tag serde).
        detail: String,
    },
    /// Prośba wygasła albo unieważniona (kill-switch).
    ApprovalExpired {
        /// Prośba.
        approval: ApprovalId,
    },
    /// Brak decyzji w limicie czasu narzędzia.
    ApprovalTimeout {
        /// Prośba.
        approval: ApprovalId,
    },
    /// Token odrzucony przy weryfikacji (zakres, podmiot, wygaśnięcie).
    TokenRejected,
    /// Audyt niedostępny — Broker nie wydaje tokenów (fail-closed).
    AuditUnavailable,
    /// Polityka narzędzia (np. polecenie sieciowe bez jawnego hosta, zakres za duży na snapshot).
    Policy,
}

impl DenialReason {
    /// Opis po polsku dla modelu i UI.
    pub fn describe(&self) -> String {
        match self {
            Self::KernelBlock { rule } => format!("blokada Jądra — {}", rule.description_pl()),
            Self::DenyList => {
                "ścieżka lub polecenie na deny-liście (poświadczenia, dane Jądra)".into()
            }
            Self::OwnerDenied { .. } => "właściciel odmówił zgody".into(),
            Self::JudgeDenied { detail, .. } => format!(
                "sędzia zgód odmówił ({detail}). Nie obchodź tej blokady innym poleceniem ani narzędziem — \
                 wybierz wyraźnie bezpieczniejszą drogę albo zapytaj właściciela"
            ),
            Self::ApprovalExpired { .. } => {
                "prośba o zgodę wygasła albo została unieważniona".into()
            }
            Self::ApprovalTimeout { .. } => "brak decyzji właściciela w wyznaczonym czasie".into(),
            Self::TokenRejected => "Sędzia Cieni odrzucił token zdolności".into(),
            Self::AuditUnavailable => "audyt niedostępny — Sędzia Cieni nie wydaje zgód".into(),
            Self::Policy => "zasady narzędzia nie pozwalają na tę akcję".into(),
        }
    }
}
