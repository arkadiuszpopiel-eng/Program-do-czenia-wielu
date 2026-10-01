//! Runner testów chaosowych (ACCEPTANCE F8-01, F8-06): każda awaria z katalogu — wstrzyknięta,
//! wykryta (poprawna klasyfikacja), naprawiona (sonda zdrowia), cofnięta (stan 1:1 jak po awarii).

use std::fmt::Write as _;
use std::future::Future;
use std::sync::Arc;

use diagnostician_contract::{
    Consent, Diagnostician, FailureKind, JournalEvent, RepairPolicy, RepairStatus, UserConsent,
};
use serde::Serialize;
use watchdog_contract::{Clock, ManualClock};

use crate::catalog::FAULTS;
use crate::faults::inject;
use crate::probe::healthy;
use crate::world::ChaosWorld;

/// Parametry budowy Diagnosty dla jednej awarii.
pub struct ChaosSetup {
    /// Świat (środowisko, kontekst, Broker: [`crate::ChaosBroker`]).
    pub world: Arc<ChaosWorld>,
    /// Zegar wspólny świata i Diagnosty.
    pub clock: Arc<ManualClock>,
    /// Polityka.
    pub policy: RepairPolicy,
}

/// Wynik jednej awarii.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FaultResult {
    /// Awaria.
    pub id: String,
    /// Rodzaj.
    pub kind: FailureKind,
    /// Wykryta z poprawnym rodzajem i celem (jedyny incydent).
    pub detected: bool,
    /// Propozycja kompletna (diff, uzasadnienie, ryzyko, plan cofnięcia).
    pub complete_proposal: bool,
    /// Kto zatwierdzał.
    pub consent: Option<Consent>,
    /// Naprawiona i zweryfikowana sondą.
    pub repaired: bool,
    /// Cofnięcie przywróciło stan sprzed naprawy 1:1.
    pub reversible: bool,
    /// Obszar Jądra wyłącznie przez Brokera (dla pozostałych: brak kroków Brokera).
    pub broker_rule_held: bool,
    /// Uwagi.
    pub notes: Vec<String>,
}

impl FaultResult {
    /// Czy wszystko zaliczone.
    pub fn ok(&self) -> bool {
        self.detected
            && self.complete_proposal
            && self.repaired
            && self.reversible
            && self.broker_rule_held
    }
}

/// Raport.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ChaosReport {
    /// Wyniki.
    pub results: Vec<FaultResult>,
}

impl ChaosReport {
    /// Czy wszystkie awarie zaliczone.
    pub fn all_ok(&self) -> bool {
        !self.results.is_empty() && self.results.iter().all(FaultResult::ok)
    }

    /// Tabela Markdown.
    pub fn to_markdown(&self) -> String {
        let mut out = String::from(
            "| Awaria | Rodzaj | Wykryta | Zgoda | Naprawiona | Cofalna | Broker | Karta F8-06 |\n|---|---|---|---|---|---|---|---|\n",
        );
        let yn = |b: bool| if b { "tak" } else { "**NIE**" };
        for r in &self.results {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {:?} | {} | {} | {} | {} |",
                r.id,
                r.kind.title(),
                yn(r.detected),
                r.consent.unwrap_or(Consent::Human),
                yn(r.repaired),
                yn(r.reversible),
                yn(r.broker_rule_held),
                yn(r.complete_proposal)
            );
        }
        out
    }
}

/// Uruchamia awarie o podanych identyfikatorach (pusta lista = cały katalog).
pub async fn run_chaos<F, Fut>(factory: F, ids: &[String]) -> ChaosReport
where
    F: Fn(ChaosSetup) -> Fut,
    Fut: Future<Output = Arc<dyn Diagnostician>>,
{
    let mut report = ChaosReport::default();
    for spec in FAULTS
        .iter()
        .filter(|s| ids.is_empty() || ids.iter().any(|i| i == s.id))
    {
        let clock = Arc::new(ManualClock::new(1_700_000_000_000));
        let world = ChaosWorld::baseline(Arc::clone(&clock));
        let mut r = FaultResult {
            id: spec.id.into(),
            kind: spec.kind,
            detected: false,
            complete_proposal: false,
            consent: None,
            repaired: false,
            reversible: false,
            broker_rule_held: false,
            notes: Vec::new(),
        };
        let signals = match inject(&world, spec.id) {
            Ok(s) => s,
            Err(e) => {
                r.notes.push(e);
                report.results.push(r);
                continue;
            }
        };
        let faulted = world.snapshot();
        let policy = RepairPolicy {
            autonomy: diagnostician_contract::RepairAutonomy::AutoMediumRisk,
            ..RepairPolicy::default()
        };
        let diag = factory(ChaosSetup {
            world: Arc::clone(&world),
            clock: Arc::clone(&clock),
            policy,
        })
        .await;
        for s in signals {
            diag.ingest(s).await;
            clock.advance(10);
        }
        diag.scan().await;
        let records = diag.repairs();
        let Some(rec) = records
            .iter()
            .find(|x| x.detection.kind == spec.kind && x.detection.target == spec.target)
            .cloned()
        else {
            r.notes.push(format!(
                "wykrycia: {:?}",
                records
                    .iter()
                    .map(|x| (x.detection.kind, x.detection.target.clone()))
                    .collect::<Vec<_>>()
            ));
            report.results.push(r);
            continue;
        };
        r.detected = records.len() == 1;
        r.complete_proposal = rec.proposal.is_complete();
        r.consent = Some(rec.consent);
        let mut status = rec.status.clone();
        if status == RepairStatus::Proposed {
            match diag
                .approve(
                    rec.id,
                    UserConsent {
                        surface: "test-chaos".into(),
                    },
                )
                .await
            {
                Ok(done) => status = done.status,
                Err(e) => r.notes.push(e.to_string()),
            }
        }
        r.repaired = status == RepairStatus::Verified
            && healthy(&world.snapshot(), &rec.detection, clock.now_ms());
        let (env, broker) = (world.env_log(), world.broker_log());
        r.broker_rule_held = if spec.kind.kernel_only() {
            env.is_empty() && !broker.is_empty() && rec.consent == Consent::Broker
        } else {
            broker.is_empty()
        };
        if !r.repaired {
            r.notes.push(format!("stan naprawy: {status:?}"));
            report.results.push(r);
            continue;
        }
        match diag.undo(rec.id).await {
            Ok(done) => {
                let clean = diag.journal().last().is_some_and(|e| matches!(&e.event, JournalEvent::Undone { errors, .. } if errors.is_empty()));
                r.reversible =
                    done.status == RepairStatus::Undone && clean && world.snapshot() == faulted;
                if !r.reversible {
                    r.notes
                        .push("stan po cofnięciu różni się od stanu po awarii".into());
                }
            }
            Err(e) => r.notes.push(e.to_string()),
        }
        report.results.push(r);
    }
    report
}
