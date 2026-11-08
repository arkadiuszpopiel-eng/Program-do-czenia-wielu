//! Zdarzenia `residency.*` (ładunki bez treści użytkownika).

use core_bus_contract::EventKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::table::Change;
use crate::types::{Device, Grant, Lease, LeaseId, Mode, Revocation};

/// Przyznano dzierżawę.
pub const EVENT_GRANTED: &str = "residency.granted";
/// Zwolniono dzierżawę.
pub const EVENT_RELEASED: &str = "residency.released";
/// Odebrano dzierżawę (kto, dlaczego).
pub const EVENT_EVICTED: &str = "residency.evicted";
/// Przeniesiono dzierżawę z GPU na CPU.
pub const EVENT_MOVED: &str = "residency.moved";
/// Zmiana trybu (gra/bateria/emulacja).
pub const EVENT_MODE_CHANGED: &str = "residency.mode_changed";
/// Eksmisja lub CPU zamiast cichego OOM.
pub const EVENT_OOM_AVOIDED: &str = "residency.oom_avoided";
/// Model większy niż budżet maszyny.
pub const EVENT_BUDGET_EXCEEDED: &str = "residency.budget_exceeded";

/// Rodzaj zdarzenia jako `EventKind` magistrali.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Zdarzenie zarządcy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ResidencyEvent {
    /// `residency.granted`.
    Granted {
        /// Dzierżawa.
        lease: Lease,
    },
    /// `residency.released`.
    Released {
        /// Dzierżawa.
        lease: Lease,
    },
    /// `residency.evicted`.
    Evicted {
        /// Odebrana dzierżawa i powód.
        revocation: Revocation,
    },
    /// `residency.moved`.
    Moved {
        /// Dzierżawa po przeniesieniu.
        lease: Lease,
        /// Nowe urządzenie.
        to: Device,
    },
    /// `residency.mode_changed`.
    ModeChanged {
        /// Poprzedni tryb.
        from: Mode,
        /// Nowy tryb.
        to: Mode,
    },
    /// `residency.oom_avoided`.
    OomAvoided {
        /// Model, dla którego zrobiono miejsce.
        model: String,
        /// Nowa dzierżawa.
        lease: LeaseId,
        /// Umiejscowienie (CPU = zamiast GPU).
        device: Device,
        /// Liczba eksmitowanych dzierżaw.
        evicted: usize,
    },
    /// `residency.budget_exceeded`.
    BudgetExceeded {
        /// Model.
        model: String,
        /// Budżet VRAM (MB).
        vram_mb: u32,
        /// Budżet RAM (MB).
        ram_mb: u32,
    },
}

impl ResidencyEvent {
    /// Nazwa zdarzenia na magistrali.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Granted { .. } => EVENT_GRANTED,
            Self::Released { .. } => EVENT_RELEASED,
            Self::Evicted { .. } => EVENT_EVICTED,
            Self::Moved { .. } => EVENT_MOVED,
            Self::ModeChanged { .. } => EVENT_MODE_CHANGED,
            Self::OomAvoided { .. } => EVENT_OOM_AVOIDED,
            Self::BudgetExceeded { .. } => EVENT_BUDGET_EXCEEDED,
        }
    }

    /// Zdarzenia opisujące udany `acquire` (eksmisje, przyznanie, ewentualne „OOM uniknięty").
    pub fn from_grant(grant: &Grant, preferred_gpu: bool) -> Vec<ResidencyEvent> {
        let mut out: Vec<ResidencyEvent> = grant
            .evicted
            .iter()
            .map(|r| Self::Evicted {
                revocation: r.clone(),
            })
            .collect();
        let fell_back = preferred_gpu && grant.lease.device == Device::Cpu;
        if !grant.evicted.is_empty() || fell_back {
            out.push(Self::OomAvoided {
                model: grant.lease.request.model.clone(),
                lease: grant.lease.id,
                device: grant.lease.device,
                evicted: grant.evicted.len(),
            });
        }
        out.push(Self::Granted {
            lease: grant.lease.clone(),
        });
        out
    }

    /// Zdarzenia opisujące zmiany trybu/budżetu.
    pub fn from_changes(changes: &[Change]) -> Vec<ResidencyEvent> {
        changes
            .iter()
            .map(|c| match c {
                Change::Revoked(r) => Self::Evicted {
                    revocation: r.clone(),
                },
                Change::Moved(l) => Self::Moved {
                    lease: l.clone(),
                    to: l.device,
                },
            })
            .collect()
    }
}
