//! DTO umiejętności (biblioteka, propozycje z podglądem diffu i hashem, kwarantanna) i Kreatora
//! agentów (szkic → podgląd persony → test na sucho → zapis nie-głosem) — odpowiedniki
//! `types-work.ts`. Szkic Kreatora to `agent_builder_contract::AgentDraft` (ten sam kształt serde).

use serde::{Deserialize, Serialize};

pub use agent_builder_contract::AgentDraft;

use super::common::{AutonomyLevel, Iso8601};

/// Stan wersji umiejętności.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillStateView {
    Proposed,
    Quarantined,
    Installed,
    Rejected,
    Disabled,
    Superseded,
}

/// Pochodzenie umiejętności.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillOrigin {
    Memory,
    User,
    OwnPackage,
    External,
}

/// Wersja umiejętności w bibliotece.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInfo {
    pub id: String,
    pub version: String,
    pub name: String,
    pub description: String,
    pub state: SkillStateView,
    pub origin: SkillOrigin,
    pub trusted: bool,
    pub hash: String,
    pub findings: Vec<String>,
    pub keywords: Vec<String>,
    pub required_tools: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub parameters: serde_json::Value,
    pub proposed_at: Iso8601,
    pub decided_at: Option<Iso8601>,
}

/// Rodzaj linii diffu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffKind {
    Same,
    Added,
    Removed,
}

/// Linia diffu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffKind,
    pub text: String,
}

/// Przegląd wersji przed zatwierdzeniem: diff względem zainstalowanej + hash do potwierdzenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillReview {
    pub skill: SkillInfo,
    pub previous_version: Option<String>,
    pub diff: Vec<DiffLine>,
}

/// Wynik importu paczki umiejętności (zawsze propozycje, nigdy instalacja).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillImportResult {
    pub proposed: Vec<SkillInfo>,
    pub skipped: Vec<String>,
}

/// Polityka Kreatora (formularz).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderPolicyView {
    pub groups: Vec<String>,
    pub ceiling: AutonomyLevel,
    pub palette: Vec<String>,
    pub voices: Vec<String>,
    pub model_policies: Vec<String>,
    pub max_steps: u32,
}

/// Szkic z rozmowy + pytania o braki.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuilderProposal {
    pub draft: AgentDraft,
    pub questions: Vec<String>,
}

/// Podgląd persony i roli zbudowanego manifestu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderPreview {
    pub hash: String,
    pub persona_id: String,
    pub name: String,
    pub forms: Vec<String>,
    pub glyph: String,
    pub color: String,
    pub character: String,
    pub role_id: String,
    pub role_name: String,
    pub groups: Vec<String>,
    pub read_only: bool,
    pub tools: Vec<String>,
    pub voice: String,
    pub autonomy: AutonomyLevel,
    pub system_prompt: String,
    pub fs_write: Vec<String>,
    pub memory_scope: String,
    pub retain_days: u32,
    pub max_steps: u32,
    pub warnings: Vec<String>,
}

/// Oczekiwany / przewidywany wynik kroku testu na sucho.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DryOutcome {
    Allowed,
    Ask,
    Denied,
}

/// Krok testu na sucho.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderDryStep {
    pub tool: String,
    pub expected: DryOutcome,
    pub outcome: DryOutcome,
    pub why: String,
}

/// Raport testu na sucho (zaliczony = warunek zapisu tego samego hasha).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderDryRun {
    pub hash: String,
    pub passed: bool,
    pub steps: Vec<BuilderDryStep>,
}

/// Zapisana agentka.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderSaved {
    pub persona: String,
    pub role: String,
    pub hash: String,
}

/// Agentka z biblioteki Kreatora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuilderAgentInfo {
    pub persona: String,
    pub name: String,
    pub color: String,
    pub role: String,
    pub autonomy: AutonomyLevel,
    pub hash: String,
}
