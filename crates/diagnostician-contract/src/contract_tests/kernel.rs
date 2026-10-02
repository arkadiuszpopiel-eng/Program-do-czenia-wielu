//! Przypadki kontraktowe: obszar Jądra, klucze zakazane, konflikty, raport i dziennik.

use std::future::Future;

use serde_json::json;

use super::cases::{consent, gpu_lost, setup, symptom};
use super::{BrokerMode, DiagHarness, DiagSetup};
use crate::{
    DiagError, FailureKind, JournalEvent, ModuleCondition, Overall, RepairAutonomy, RepairStatus,
    Signal, Symptom, WatchdogSignal,
};

/// Obszar Jądra: tylko przez Brokera; odmowa = człowiek; zgoda użytkownika nie wystarcza.
pub(super) async fn kernel_area_only_through_broker<F, Fut>(factory: &F)
where
    F: Fn(DiagSetup) -> Fut,
    Fut: Future<Output = DiagHarness>,
{
    let pkg = "jadro/wersje/0.0.2.zip";
    let s = setup(&[], &[pkg], RepairAutonomy::AutoMediumRisk);
    let (world, broker) = (s.world.clone(), s.broker.clone());
    broker.set_mode(BrokerMode::Deny);
    let h = factory(s).await;
    h.diag
        .ingest(symptom(
            "updater",
            Symptom::UpdateSignatureInvalid,
            pkg,
            &[],
        ))
        .await;
    let out = h.diag.scan().await;
    assert_eq!(out.needs_human.len(), 1, "{out:?}");
    assert_eq!(broker.seen().len(), 1);
    assert!(
        world.snapshot().files.contains(pkg),
        "Diagnosta nie wykonała kroków sama"
    );
    let id = out.needs_human[0];
    assert!(matches!(
        h.diag.approve(id, consent()).await,
        Err(DiagError::KernelAreaRequiresBroker(_))
    ));

    let s = setup(&[], &[pkg], RepairAutonomy::ProposeOnly);
    let (world, broker) = (s.world.clone(), s.broker.clone());
    let h = factory(s).await;
    h.diag
        .ingest(symptom(
            "updater",
            Symptom::UpdateSignatureInvalid,
            pkg,
            &[],
        ))
        .await;
    h.diag
        .ingest(Signal::ModuleState {
            module: "safety-broker".into(),
            condition: ModuleCondition::Failed { restarts: 2 },
            detail: "x".into(),
        })
        .await;
    let out = h.diag.scan().await;
    assert_eq!(
        out.repaired.len(),
        2,
        "Broker wykonuje także przy ProposeOnly: {out:?}"
    );
    assert!(broker.seen().iter().all(|p| p.kernel_area));
    assert!(
        world
            .snapshot()
            .files
            .contains("kwarantanna/jadro/wersje/0.0.2.zip")
    );
    let pkg_id = h
        .diag
        .repairs()
        .iter()
        .find(|r| r.detection.kind == FailureKind::UpdatePackageCorrupted)
        .map(|r| r.id)
        .unwrap_or_else(|| panic!("brak naprawy paczki"));
    let rec = h.diag.undo(pkg_id).await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(rec.status, RepairStatus::Undone);
    assert!(world.snapshot().files.contains(pkg));
    assert!(
        world.snapshot().config.is_empty(),
        "moduł Jądra nigdy nie jest wyłączany"
    );
}

/// Klucze zakazane z niezaufanych szczegółów sygnału; konflikt przy cofnięciu nie nadpisuje.
pub(super) async fn forbidden_keys_and_conflicts<F, Fut>(factory: &F)
where
    F: Fn(DiagSetup) -> Fut,
    Fut: Future<Output = DiagHarness>,
{
    let s = setup(
        &[("diagnostician.autonomy", json!("auto_low_risk"))],
        &[],
        RepairAutonomy::AutoMediumRisk,
    );
    let world = s.world.clone();
    let h = factory(s).await;
    h.diag.ingest(gpu_lost("diagnostician.autonomy")).await;
    let out = h.diag.scan().await;
    assert_eq!(
        world.snapshot().config["diagnostician.autonomy"],
        json!("auto_low_risk")
    );
    // Klucz zakazany z sygnału jest ignorowany (SR2-03): naprawa — jeśli jest — dotyczy
    // wyłącznie klucza domyślnego modułu, nigdy `diagnostician.*`.
    assert!(
        h.diag.repairs().iter().all(|r| r
            .proposal
            .steps
            .iter()
            .filter_map(crate::RepairStep::config_key)
            .all(|k| !k.starts_with("diagnostician"))),
        "{out:?}"
    );

    // Klucz `kernel.*` podsunięty sygnałem (niezaufanym) nie trafia ani do Diagnosty, ani do
    // Brokera — zostaje klucz domyślny modułu (przegląd #2, SR2-03).
    let s = setup(&[], &[], RepairAutonomy::AutoMediumRisk);
    let (world, broker) = (s.world.clone(), s.broker.clone());
    broker.set_mode(BrokerMode::Pending);
    let h = factory(s).await;
    h.diag.ingest(gpu_lost("kernel.gpu.device")).await;
    let out = h.diag.scan().await;
    assert!(
        out.kernel_pending.is_empty() && broker.seen().is_empty(),
        "klucz kernel.* z sygnału ignorowany: {out:?}"
    );
    assert!(!world.snapshot().config.contains_key("kernel.gpu.device"));

    let s = setup(
        &[("router.offline", json!(false))],
        &[],
        RepairAutonomy::AutoLowRisk,
    );
    let world = s.world.clone();
    let h = factory(s).await;
    for _ in 0..2 {
        h.diag
            .ingest(symptom("router", Symptom::NetworkUnreachable, "x", &[]))
            .await;
    }
    let id = h.diag.scan().await.repaired[0];
    world.set_config("router.offline", json!("ręcznie"));
    let rec = h.diag.undo(id).await.unwrap_or_else(|e| panic!("{e}"));
    assert!(
        matches!(h.diag.journal().last().map(|e| &e.event), Some(JournalEvent::Undone { errors, .. }) if !errors.is_empty())
    );
    assert_eq!(rec.status, RepairStatus::Undone);
    assert_eq!(
        world.snapshot().config["router.offline"],
        json!("ręcznie"),
        "konflikt nie nadpisany"
    );
}

/// Raport (safe-mode, moduły, potrzeby człowieka) i dziennik append-only bez luk.
pub(super) async fn report_and_journal<F, Fut>(factory: &F)
where
    F: Fn(DiagSetup) -> Fut,
    Fut: Future<Output = DiagHarness>,
{
    let h = factory(setup(&[], &[], RepairAutonomy::AutoLowRisk)).await;
    assert_eq!(h.diag.report().overall, Overall::Ok);
    h.diag
        .ingest(Signal::ModuleState {
            module: "voice-tts".into(),
            condition: ModuleCondition::Degraded,
            detail: "wolno".into(),
        })
        .await;
    assert_eq!(h.diag.report().overall, Overall::Degraded);
    h.diag
        .ingest(symptom(
            "providers-api",
            Symptom::Http { status: 401 },
            "anthropic",
            &[],
        ))
        .await;
    let out = h.diag.scan().await;
    assert_eq!(out.repaired.len(), 1);
    let before = h.diag.journal();
    let report = h.diag.report();
    assert_eq!(report.repaired.len(), 1);
    assert!(
        report
            .needs_human
            .iter()
            .any(|n| n.mitigated && n.what.contains("klucz"))
    );
    assert_eq!(report.incidents[0].kind, FailureKind::ApiKeyRevoked);
    h.diag
        .ingest(Signal::Watchdog(WatchdogSignal::SafeModeEntered {
            reason: "pętla awarii".into(),
        }))
        .await;
    assert_eq!(h.diag.report().overall, Overall::SafeMode);
    h.diag
        .undo(out.repaired[0])
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let after = h.diag.journal();
    assert_eq!(
        &after[..before.len()],
        &before[..],
        "dziennik tylko dopisywany"
    );
    assert!(
        after
            .iter()
            .enumerate()
            .all(|(i, e)| e.seq == u64::try_from(i).unwrap_or(0) + 1)
    );
}
