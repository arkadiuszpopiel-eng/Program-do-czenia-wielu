//! DTO strony „Zdrowie systemu": stan modułów (rejestr + Diagnosta), incydenty, naprawy cofalne,
//! propozycje Diagnosty i Ulepszacza (diff, ryzyko, plan cofnięcia), wyniki evali (werdykty
//! zbiorcze, integralność zestawów) — odpowiedniki `types-work.ts`.

use serde::{Deserialize, Serialize};

use super::common::Iso8601;

/// Stan ogólny.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthOverall {
    Ok,
    Degraded,
    Failing,
    SafeMode,
}

/// Stan modułu w rejestrze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleHealth {
    Healthy,
    Degraded,
    Unhealthy,
    NotStarted,
    Disabled,
}

/// Wiersz modułu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthModule {
    pub module: String,
    pub version: String,
    pub lifecycle: String,
    pub health: ModuleHealth,
    pub detail: Option<String>,
}

/// Incydent Diagnosty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthIncident {
    pub id: u64,
    pub kind: String,
    pub title: String,
    pub target: String,
    pub count: u32,
    pub last_at: Iso8601,
    pub status: String,
}

/// Wykonana naprawa (z „Cofnij").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthRepair {
    pub id: u64,
    pub title: String,
    pub diff: Vec<String>,
    pub at: Iso8601,
    pub undoable: bool,
}

/// Co wymaga człowieka.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthHumanAction {
    pub id: u64,
    pub title: String,
    pub what: String,
    pub mitigated: bool,
}

/// Ryzyko zmiany.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskView {
    Low,
    Medium,
    High,
}

/// Propozycja naprawy czekająca na zgodę (Jądro → wyłącznie Broker-UI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthProposal {
    pub id: u64,
    pub title: String,
    pub diff: Vec<String>,
    pub rationale: String,
    pub risk: RiskView,
    pub rollback_plan: Vec<String>,
    pub kernel: bool,
}

/// Raport „Zdrowie systemu".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthView {
    pub overall: HealthOverall,
    pub safe_mode: Option<String>,
    pub generated_at: Iso8601,
    pub modules: Vec<HealthModule>,
    pub incidents: Vec<HealthIncident>,
    pub repaired: Vec<HealthRepair>,
    pub needs_human: Vec<HealthHumanAction>,
    pub pending: Vec<HealthProposal>,
    pub problems: Vec<String>,
}

/// Zmiana konfiguracji w propozycji Ulepszacza.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImproverChange {
    pub key: String,
    pub old: Option<serde_json::Value>,
    pub new: serde_json::Value,
    pub ring: String,
    pub safety: String,
}

/// Propozycja Ulepszacza.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImproverProposalView {
    pub id: u64,
    pub title: String,
    pub rationale: String,
    pub source: String,
    pub ring: String,
    pub safety: String,
    pub stage: String,
    pub note: Option<String>,
    pub digest: String,
    pub changes: Vec<ImproverChange>,
    pub created_at: Iso8601,
    pub needs_signature: bool,
    pub can_approve: bool,
    pub can_rollback: bool,
}

/// Zablokowana próba (bez treści wartości).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImproverBlocked {
    pub at: Iso8601,
    pub source: String,
    pub target: String,
    pub violation: String,
}

/// Szkic zgłoszenia R3 (kod — tylko do przeglądu człowieka).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImproverIssue {
    pub at: Iso8601,
    pub title: String,
    pub path: String,
    pub body: String,
}

/// Panel Ulepszacza.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImproverView {
    pub proposals: Vec<ImproverProposalView>,
    pub blocked: Vec<ImproverBlocked>,
    pub issues: Vec<ImproverIssue>,
    pub idle_cycle: bool,
    pub last_cycle: Option<Iso8601>,
}

/// Zestaw ewaluacyjny (bez przypadków).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalSuiteView {
    pub id: String,
    pub wave: String,
    pub version: String,
    pub status: String,
    pub integrity_ok: bool,
    pub problems: Vec<String>,
    pub thresholds: u32,
}

/// Werdykt bramki (wynik zbiorczy — bez danych przypadków holdoutu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalVerdictView {
    pub at: Iso8601,
    pub suite: String,
    pub stage: String,
    pub passed: bool,
    pub summary: String,
}

/// Wyniki evali.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalsView {
    pub available: bool,
    pub reason: Option<String>,
    pub suites: Vec<EvalSuiteView>,
    pub holdout_suites: u32,
    pub verdicts: Vec<EvalVerdictView>,
}
