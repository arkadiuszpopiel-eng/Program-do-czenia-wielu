//! Raport „Zdrowie systemu” (API dla UI): stan modułów, ostatnie incydenty, co naprawiono,
//! co wymaga człowieka, propozycje czekające na zgodę.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::{FailureKind, Severity};
use crate::journal::{Consent, RepairId, RepairRecord, RepairStatus};
use crate::plan::Risk;
use crate::signal::{ModuleCondition, Resource};

/// Stan ogólny.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Overall {
    /// Wszystko działa.
    Ok,
    /// Działa z ograniczeniami.
    Degraded,
    /// Poważna awaria bez naprawy.
    Failing,
    /// Safe-mode watchdoga.
    SafeMode,
}

/// Pomiar zasobu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Usage {
    /// Zasób.
    pub resource: Resource,
    /// Wartość.
    pub used: u64,
    /// Limit.
    pub limit: u64,
}

/// Wiersz modułu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModuleRow {
    /// Moduł.
    pub module: String,
    /// Stan.
    pub condition: ModuleCondition,
    /// Opis.
    pub detail: String,
    /// Ostatnie pomiary zasobów.
    pub usage: Vec<Usage>,
}

/// Incydent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IncidentRow {
    /// Naprawa.
    pub id: RepairId,
    /// Rodzaj.
    pub kind: FailureKind,
    /// Tytuł.
    pub title: String,
    /// Cel.
    pub target: String,
    /// Liczba sygnałów.
    pub count: usize,
    /// Ostatni sygnał (ms).
    pub last_ms: u64,
    /// Stan naprawy.
    pub status: String,
}

/// Wykonana naprawa.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepairRow {
    /// Naprawa.
    pub id: RepairId,
    /// Tytuł.
    pub title: String,
    /// Diff.
    pub diff: Vec<String>,
    /// Kiedy (ms).
    pub at_ms: u64,
    /// Czy można cofnąć.
    pub undoable: bool,
}

/// Co wymaga człowieka.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HumanAction {
    /// Naprawa.
    pub id: RepairId,
    /// Tytuł.
    pub title: String,
    /// Co zrobić.
    pub what: String,
    /// Czy system już działa (naprawa tymczasowa wykonana).
    pub mitigated: bool,
}

/// Karta propozycji czekającej na zgodę (diff, uzasadnienie, ryzyko, plan cofnięcia).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProposalCard {
    /// Naprawa.
    pub id: RepairId,
    /// Tytuł.
    pub title: String,
    /// Diff.
    pub diff: Vec<String>,
    /// Uzasadnienie.
    pub rationale: String,
    /// Ryzyko.
    pub risk: Risk,
    /// Plan cofnięcia.
    pub rollback_plan: Vec<String>,
    /// Kto zatwierdza.
    pub consent: Consent,
}

/// Raport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HealthReport {
    /// Wygenerowano (ms).
    pub generated_ms: u64,
    /// Stan ogólny.
    pub overall: Overall,
    /// Powód safe-mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safe_mode: Option<String>,
    /// Moduły.
    pub modules: Vec<ModuleRow>,
    /// Ostatnie incydenty (najnowsze pierwsze).
    pub incidents: Vec<IncidentRow>,
    /// Co naprawiono.
    pub repaired: Vec<RepairRow>,
    /// Co wymaga człowieka.
    pub needs_human: Vec<HumanAction>,
    /// Propozycje czekające na zgodę (użytkownik albo Broker).
    pub pending: Vec<ProposalCard>,
}

/// Składa raport.
pub fn build_report(
    now_ms: u64,
    modules: &BTreeMap<String, ModuleRow>,
    safe_mode: Option<&String>,
    records: &[RepairRecord],
    limit: usize,
) -> HealthReport {
    let mut recent: Vec<&RepairRecord> = records.iter().collect();
    recent.sort_by_key(|r| std::cmp::Reverse((r.updated_ms, r.id)));
    let open_serious = recent.iter().any(|r| {
        r.status.is_open()
            && r.status != RepairStatus::Applied
            && r.detection.kind.severity() >= Severity::High
    });
    let degraded = recent.iter().any(|r| r.status.is_open())
        || modules
            .values()
            .any(|m| m.condition != ModuleCondition::Ready);
    let overall = match (safe_mode.is_some(), open_serious, degraded) {
        (true, _, _) => Overall::SafeMode,
        (false, true, _) => Overall::Failing,
        (false, false, true) => Overall::Degraded,
        _ => Overall::Ok,
    };
    let title = |r: &RepairRecord| r.proposal.title.clone();
    HealthReport {
        generated_ms: now_ms,
        overall,
        safe_mode: safe_mode.cloned(),
        modules: modules.values().cloned().collect(),
        incidents: recent
            .iter()
            .take(limit)
            .map(|r| IncidentRow {
                id: r.id,
                kind: r.detection.kind,
                title: title(r),
                target: r.detection.target.clone(),
                count: r.detection.count,
                last_ms: r.detection.last_ms,
                status: r.status.name().to_owned(),
            })
            .collect(),
        repaired: recent
            .iter()
            .filter(|r| r.status == RepairStatus::Verified)
            .take(limit)
            .map(|r| RepairRow {
                id: r.id,
                title: title(r),
                diff: r.proposal.diff.clone(),
                at_ms: r.updated_ms,
                undoable: r.receipts.iter().any(|s| s.is_stateful()),
            })
            .collect(),
        needs_human: recent
            .iter()
            .filter_map(|r| match (&r.status, &r.proposal.needs_human) {
                (RepairStatus::NeedsHuman { reason }, _) => Some((r, reason.clone(), false)),
                (RepairStatus::Verified, Some(what)) => Some((r, what.clone(), true)),
                _ => None,
            })
            .take(limit)
            .map(|(r, what, mitigated)| HumanAction {
                id: r.id,
                title: title(r),
                what,
                mitigated,
            })
            .collect(),
        pending: recent
            .iter()
            .filter(|r| {
                matches!(
                    r.status,
                    RepairStatus::Proposed | RepairStatus::KernelPending { .. }
                )
            })
            .take(limit)
            .map(|r| ProposalCard {
                id: r.id,
                title: title(r),
                diff: r.proposal.diff.clone(),
                rationale: r.proposal.rationale.clone(),
                risk: r.proposal.risk,
                rollback_plan: r.proposal.rollback_plan.clone(),
                consent: r.consent,
            })
            .collect(),
    }
}
