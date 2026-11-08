//! Dziennik napraw (append-only) i rekordy napraw. Każda naprawa: propozycja → (zgoda) →
//! wykonanie z pokwitowaniami → weryfikacja → ewentualne cofnięcie (operacje odwrotne).

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::FailureKind;
use crate::classify::Detection;
use crate::plan::Proposal;
use crate::step::RepairStep;

/// Identyfikator naprawy (= incydentu i propozycji).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct RepairId(pub u64);

impl fmt::Display for RepairId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "N-{}", self.0)
    }
}

/// Kto musi wyrazić zgodę.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Consent {
    /// Automatycznie (wg autonomii Diagnosty).
    Auto,
    /// Użytkownik (panel „Zdrowie systemu”).
    User,
    /// Broker (obszar Jądra, Broker-UI z fizycznym potwierdzeniem).
    Broker,
    /// Brak automatycznej naprawy — tylko człowiek.
    Human,
}

/// Stan naprawy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RepairStatus {
    /// Czeka na zgodę.
    Proposed,
    /// Wykonana, przed weryfikacją.
    Applied,
    /// Wykonana i zweryfikowana.
    Verified,
    /// Nieudana — kroki cofnięte.
    Failed {
        /// Powód.
        reason: String,
    },
    /// Cofnięta na żądanie.
    Undone,
    /// Odrzucona.
    Rejected,
    /// Wymaga człowieka.
    NeedsHuman {
        /// Co zrobić.
        reason: String,
    },
    /// Czeka w Broker-UI.
    KernelPending {
        /// Bilet.
        ticket: String,
    },
}

impl RepairStatus {
    /// Nazwa jak w JSON.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Applied => "applied",
            Self::Verified => "verified",
            Self::Failed { .. } => "failed",
            Self::Undone => "undone",
            Self::Rejected => "rejected",
            Self::NeedsHuman { .. } => "needs_human",
            Self::KernelPending { .. } => "kernel_pending",
        }
    }

    /// Czy incydent jest otwarty (blokuje nową propozycję dla tego samego celu).
    pub fn is_open(&self) -> bool {
        matches!(
            self,
            Self::Proposed | Self::Applied | Self::NeedsHuman { .. } | Self::KernelPending { .. }
        )
    }
}

/// Zdarzenie dziennika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum JournalEvent {
    /// Incydent i propozycja.
    Proposed {
        /// Wykrycie.
        detection: Box<Detection>,
        /// Propozycja.
        proposal: Box<Proposal>,
        /// Zgoda.
        consent: Consent,
    },
    /// Zgoda użytkownika.
    Approved {
        /// Skąd.
        surface: String,
    },
    /// Kroki wykonane (pokwitowania = stan sprzed do cofnięcia).
    Applied {
        /// Pokwitowania.
        receipts: Vec<RepairStep>,
    },
    /// Weryfikacja udana.
    Verified,
    /// Kroki cofnięte po nieudanej naprawie.
    RolledBack {
        /// Wykonane kroki odwrotne.
        receipts: Vec<RepairStep>,
        /// Błędy cofania.
        errors: Vec<String>,
        /// Powód.
        reason: String,
    },
    /// Naprawa cofnięta na żądanie.
    Undone {
        /// Wykonane kroki odwrotne.
        receipts: Vec<RepairStep>,
        /// Błędy cofania.
        errors: Vec<String>,
    },
    /// Odrzucona.
    Rejected,
    /// Wymaga człowieka.
    NeedsHuman {
        /// Powód.
        reason: String,
    },
    /// Broker: czeka.
    KernelPending {
        /// Bilet.
        ticket: String,
    },
}

/// Wpis dziennika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct JournalEntry {
    /// Numer kolejny (od 1, bez luk).
    pub seq: u64,
    /// Czas (ms).
    pub ts_ms: u64,
    /// Naprawa.
    pub repair: RepairId,
    /// Rodzaj awarii.
    pub kind: FailureKind,
    /// Cel.
    pub target: String,
    /// Zdarzenie.
    pub event: JournalEvent,
}

/// Rekord naprawy (stan wyprowadzony z dziennika).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepairRecord {
    /// Identyfikator.
    pub id: RepairId,
    /// Wykrycie.
    pub detection: Detection,
    /// Propozycja.
    pub proposal: Proposal,
    /// Zgoda.
    pub consent: Consent,
    /// Stan.
    pub status: RepairStatus,
    /// Pokwitowania ostatniego wykonania.
    pub receipts: Vec<RepairStep>,
    /// Utworzono (ms).
    pub created_ms: u64,
    /// Ostatnia zmiana (ms).
    pub updated_ms: u64,
}

impl RepairRecord {
    /// Stosuje zdarzenie dziennika (odtwarzanie po restarcie i bieżąca praca).
    pub fn apply(&mut self, entry: &JournalEntry) {
        self.updated_ms = entry.ts_ms;
        match &entry.event {
            JournalEvent::Proposed { .. } | JournalEvent::Approved { .. } => {}
            JournalEvent::Applied { receipts } => {
                self.receipts.clone_from(receipts);
                self.status = RepairStatus::Applied;
            }
            JournalEvent::Verified => self.status = RepairStatus::Verified,
            JournalEvent::RolledBack { reason, .. } => {
                self.status = RepairStatus::Failed {
                    reason: reason.clone(),
                }
            }
            JournalEvent::Undone { .. } => self.status = RepairStatus::Undone,
            JournalEvent::Rejected => self.status = RepairStatus::Rejected,
            JournalEvent::NeedsHuman { reason } => {
                self.status = RepairStatus::NeedsHuman {
                    reason: reason.clone(),
                }
            }
            JournalEvent::KernelPending { ticket } => {
                self.status = RepairStatus::KernelPending {
                    ticket: ticket.clone(),
                }
            }
        }
    }
}
