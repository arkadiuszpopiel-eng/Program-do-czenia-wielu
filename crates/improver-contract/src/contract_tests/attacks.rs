//! Testy negatywne granic (ACCEPTANCE F8-02: 0 zmian w ≥ 100 próbach) i zestaw zmian R0
//! (F8-04: 0 rozszerzających wdrożonych automatycznie w 50 próbach, każde wdrożenie cofalne).
//! Zestawy są danymi w `evals/F8/improver/` (wczytywane przez testy `-impl` i `-fake`).

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use core_config_contract::{ConfigKey, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Harness, ScriptedProposer, Setup, signature_for};
use crate::{
    CandidateSet, ChangeTarget, ImproverError, MetricsSnapshot, Proposal, ProposalId, Ring,
    RunConditions, SafetyClass, Stage, UserApproval,
};

const IDLE: RunConditions = RunConditions {
    on_battery: false,
    game_mode: false,
    user_idle: true,
};

/// Droga ataku.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackVia {
    /// Bezpośrednie `submit`.
    Submit,
    /// Niezaufane źródło propozycji (model) podczas obserwacji.
    Proposer,
    /// Dozwolona zmiana + fałszywe zatwierdzenia (inny diff, brak/cudzy podpis, inna propozycja).
    ForgedApproval,
    /// Rozszerzająca zmiana R0 licząca na wdrożenie automatyczne.
    WideningAuto,
}

/// Jedna próba.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attack {
    /// Identyfikator.
    pub id: String,
    /// Technika (grupa w raporcie).
    pub technique: String,
    /// Droga.
    pub via: AttackVia,
    /// Cele.
    pub targets: Vec<ChangeTarget>,
    /// Wartości początkowe konfiguracji.
    #[serde(default)]
    pub initial: BTreeMap<String, Value>,
}

/// Zestaw prób (plik `attacks.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttackSet {
    /// ID kryterium.
    pub id: String,
    /// Wersja.
    pub version: u32,
    /// Opis.
    pub description: String,
    /// Próby.
    pub attempts: Vec<Attack>,
}

/// Wynik.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AttackReport {
    /// Liczba prób.
    pub attempts: usize,
    /// Próby zakończone jakimkolwiek zapisem Ulepszacza (musi być 0).
    pub successes: Vec<String>,
    /// Próby per technika.
    pub by_technique: BTreeMap<String, usize>,
    /// Zablokowane przez strażnika (suma wpisów `blocked`).
    pub blocked: usize,
}

fn forgeries(p: &Proposal) -> Vec<UserApproval> {
    let zero = "0".repeat(64);
    let other = "f".repeat(64);
    let mut out = vec![
        UserApproval {
            proposal: p.id,
            digest: zero.clone(),
            surface: "ulepszacz".into(),
            signature: Some(signature_for(&zero)),
        },
        UserApproval {
            proposal: ProposalId(p.id.0 + 1000),
            digest: p.digest.clone(),
            surface: "ulepszacz".into(),
            signature: Some(signature_for(&p.digest)),
        },
    ];
    if p.ring != Ring::R0 {
        out.push(UserApproval {
            proposal: p.id,
            digest: p.digest.clone(),
            surface: "ulepszacz".into(),
            signature: None,
        });
        out.push(UserApproval {
            proposal: p.id,
            digest: p.digest.clone(),
            surface: "ulepszacz".into(),
            signature: Some(signature_for(&other)),
        });
    }
    out
}

/// Uruchamia próby; każda na świeżej instancji, z bramką przepuszczającą wszystko (najgorszy przypadek).
pub async fn run_attacks<F, Fut>(factory: &F, set: &AttackSet) -> AttackReport
where
    F: Fn(Setup) -> Fut,
    Fut: Future<Output = Harness>,
{
    let mut report = AttackReport::default();
    for attack in &set.attempts {
        report.attempts += 1;
        *report
            .by_technique
            .entry(attack.technique.clone())
            .or_default() += 1;
        let mut setup = Setup::new(attack.initial.clone().into_iter().collect());
        let candidate = CandidateSet {
            title: attack.id.clone(),
            rationale: attack.technique.clone(),
            source: "atak".into(),
            targets: attack.targets.clone(),
        };
        if attack.via == AttackVia::Proposer {
            setup
                .proposers
                .push(Arc::new(ScriptedProposer::new(vec![candidate.clone()])));
        }
        let h = factory(setup).await;
        let proposals: Vec<Proposal> = match attack.via {
            AttackVia::Proposer => h
                .improver
                .observe(&MetricsSnapshot::default(), IDLE)
                .await
                .unwrap_or_default(),
            _ => h.improver.submit(candidate).await.into_iter().collect(),
        };
        for p in proposals {
            let _ = h.improver.evaluate(p.id).await;
            for forged in forgeries(&p) {
                let _ = h.improver.approve(forged).await;
            }
        }
        report.blocked += h.improver.blocked().len();
        if !h.improver_writes().is_empty() {
            report.successes.push(attack.id.clone());
        }
    }
    report
}

/// Oczekiwana klasa zmiany R0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum R0Expect {
    /// Zawężająca.
    Narrowing,
    /// Bezpieczna.
    Safe,
    /// Neutralna (przegląd).
    Neutral,
    /// Rozszerzająca.
    Widening,
    /// Odrzucona przez strażnika.
    Rejected,
}

/// Przypadek zestawu R0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R0Case {
    /// Identyfikator.
    pub id: String,
    /// Klucz.
    pub key: String,
    /// Wartość przed.
    pub old: Value,
    /// Proponowana wartość.
    pub new: Value,
    /// Oczekiwana klasa.
    pub expect: R0Expect,
}

/// Zestaw R0 (plik `r0-changes.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct R0Set {
    /// ID kryterium.
    pub id: String,
    /// Wersja.
    pub version: u32,
    /// Opis.
    pub description: String,
    /// Przypadki.
    pub cases: Vec<R0Case>,
}

/// Wynik zestawu R0.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct R0Report {
    /// Przypadki.
    pub cases: usize,
    /// Wdrożone automatycznie.
    pub auto_applied: usize,
    /// Wdrożone automatycznie, choć nie zawężające/bezpieczne (musi być 0).
    pub non_narrowing_auto_applied: Vec<String>,
    /// Wdrożone, ale rollback nie przywrócił wartości (musi być 0).
    pub not_reversible: Vec<String>,
    /// Klasa inna niż oczekiwana.
    pub misclassified: Vec<String>,
}

fn class_of(e: R0Expect) -> Option<SafetyClass> {
    match e {
        R0Expect::Narrowing => Some(SafetyClass::Narrowing),
        R0Expect::Safe => Some(SafetyClass::Safe),
        R0Expect::Neutral => Some(SafetyClass::Neutral),
        R0Expect::Widening => Some(SafetyClass::Widening),
        R0Expect::Rejected => None,
    }
}

/// Uruchamia zestaw R0.
pub async fn run_r0_cases<F, Fut>(factory: &F, set: &R0Set) -> R0Report
where
    F: Fn(Setup) -> Fut,
    Fut: Future<Output = Harness>,
{
    let mut report = R0Report::default();
    for case in &set.cases {
        report.cases += 1;
        let h = factory(Setup::new(vec![(case.key.clone(), case.old.clone())])).await;
        let candidate = CandidateSet {
            title: case.id.clone(),
            rationale: "zestaw R0".into(),
            source: "rule:f8-04".into(),
            targets: vec![ChangeTarget::Config {
                key: case.key.clone(),
                value: case.new.clone(),
            }],
        };
        let p = match h.improver.submit(candidate).await {
            Err(ImproverError::Guard(_)) => {
                if case.expect != R0Expect::Rejected {
                    report.misclassified.push(case.id.clone());
                }
                continue;
            }
            Err(e) => panic!("{}: {e}", case.id),
            Ok(p) => p,
        };
        if class_of(case.expect) != Some(p.safety) {
            report.misclassified.push(case.id.clone());
        }
        let p = h
            .improver
            .evaluate(p.id)
            .await
            .unwrap_or_else(|e| panic!("{}: {e}", case.id));
        if p.stage != (Stage::Deployed { auto: true }) {
            continue;
        }
        report.auto_applied += 1;
        if !matches!(p.safety, SafetyClass::Narrowing | SafetyClass::Safe) {
            report.non_narrowing_auto_applied.push(case.id.clone());
        }
        let _ = h.improver.rollback(p.id).await;
        let key = ConfigKey::new(case.key.as_str()).unwrap_or_else(|e| panic!("{e}"));
        let back = h.config.get(&key, &Scope::Global).await.ok().flatten();
        if back.as_ref() != Some(&case.old) {
            report.not_reversible.push(case.id.clone());
        }
    }
    report
}
