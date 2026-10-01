//! Planista napraw: wykrycie → Propozycja zmiany (diff, uzasadnienie, ryzyko, plan cofnięcia —
//! ACCEPTANCE F8-06). Kroki tylko cofalne; obszar Jądra oznaczony do Brokera.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::FailureKind;
use crate::classify::Detection;
use crate::plan_rules::{Builder, build};
use crate::ports::RepairContext;
use crate::step::{RepairStep, is_forbidden_key, is_kernel_key, is_kernel_module};

/// Ryzyko naprawy.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// Niskie (przełączniki, trasy, restart).
    Low,
    /// Średnie (pliki w kwarantannie, rollback konfiguracji, wyłączenie modułu).
    Medium,
    /// Wysokie (przywrócenie danych z kopii).
    High,
}

/// Propozycja zmiany.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Proposal {
    /// Rodzaj awarii.
    pub kind: FailureKind,
    /// Cel.
    pub target: String,
    /// Moduł.
    pub module: String,
    /// Tytuł.
    pub title: String,
    /// Uzasadnienie (hipoteza + dowody).
    pub rationale: String,
    /// Diff (linie po polsku).
    pub diff: Vec<String>,
    /// Ryzyko.
    pub risk: Risk,
    /// Plan cofnięcia (operacje odwrotne w odwrotnej kolejności).
    pub rollback_plan: Vec<String>,
    /// Kroki.
    pub steps: Vec<RepairStep>,
    /// Obszar Jądra — wykonuje wyłącznie Broker.
    pub kernel_area: bool,
    /// Co musi zrobić człowiek (także po udanej naprawie, np. „dodaj nowy klucz”).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs_human: Option<String>,
    /// Jak sprawdzimy, że pomogło.
    pub verification: String,
}

impl Proposal {
    /// Kompletność karty (F8-06): diff, uzasadnienie, ryzyko, plan cofnięcia.
    pub fn is_complete(&self) -> bool {
        !self.steps.is_empty()
            && self.diff.len() == self.steps.len()
            && !self.rationale.trim().is_empty()
            && self.rollback_plan.len() == self.steps.len()
            && !self.verification.is_empty()
    }
}

/// Segment klucza konfiguracji z dowolnego identyfikatora (`voice-stt` → `voice_stt`).
pub fn key_segment(text: &str) -> String {
    let s: String = text
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() { "x".into() } else { s }
}

/// Planuje naprawę wykrycia.
pub fn plan(d: &Detection, ctx: &dyn RepairContext) -> Proposal {
    let mut b = Builder {
        d,
        ctx,
        steps: Vec::new(),
        human: None,
        risk: Risk::Low,
        why: String::new(),
    };
    build(&mut b);
    if let Some(key) = b
        .steps
        .iter()
        .filter_map(RepairStep::config_key)
        .find(|k| is_forbidden_key(k))
    {
        b.human = Some(format!(
            "Naprawa wymagałaby zmiany `{key}` — poza zasięgiem Diagnosty."
        ));
        b.steps.clear();
    }
    let kernel_area = d.kind.kernel_only()
        || (d.kind == FailureKind::ModuleStartFailure && is_kernel_module(&d.module))
        || b.steps.iter().any(|s| match s {
            RepairStep::SetConfig { key, .. } => is_kernel_key(key),
            RepairStep::RestartModule { module } => is_kernel_module(module),
            RepairStep::MoveFile { from, to } | RepairStep::CopyFile { from, to } => {
                ctx.is_kernel_path(from) || ctx.is_kernel_path(to)
            }
            _ => false,
        });
    let evidence = d
        .evidence
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join("; ");
    Proposal {
        kind: d.kind,
        target: d.target.clone(),
        module: d.module.clone(),
        title: format!("{}: {}", d.kind.title(), d.target),
        rationale: format!("{} Dowody ({}×): {evidence}.", b.why, d.count),
        diff: b.steps.iter().map(RepairStep::describe).collect(),
        risk: b.risk,
        rollback_plan: b
            .steps
            .iter()
            .rev()
            .map(|s| {
                if s.is_stateful() {
                    s.inverse().describe()
                } else {
                    format!("{} (bez zmiany stanu)", s.describe())
                }
            })
            .collect(),
        steps: b.steps,
        kernel_area,
        needs_human: b.human,
        verification: format!(
            "ponowna sonda: „{}” nie występuje dla {}",
            d.kind.title(),
            d.target
        ),
    }
}
