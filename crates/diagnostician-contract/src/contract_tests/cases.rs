//! Przypadki kontraktowe Diagnosty.

use std::collections::BTreeMap;
use std::future::Future;

use serde_json::{Value, json};

use super::{BrokerMode, DiagHarness, DiagSetup, MiniWorld, ScriptedBroker};
use crate::{
    Consent, DiagError, JournalEvent, RepairAutonomy, RepairPolicy, RepairStatus, Risk, Signal,
    Symptom, UserConsent,
};

pub(super) fn symptom(
    module: &str,
    symptom: Symptom,
    target: &str,
    details: &[(&str, &str)],
) -> Signal {
    Signal::Error {
        module: module.into(),
        symptom,
        target: Some(target.into()),
        details: details
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect::<BTreeMap<_, _>>(),
    }
}

pub(super) fn gpu_lost(device_key: &str) -> Signal {
    symptom(
        "voice-stt",
        Symptom::GpuDeviceLost,
        "voice-stt",
        &[("device_key", device_key)],
    )
}

pub(super) fn setup(
    config: &[(&str, Value)],
    files: &[&str],
    autonomy: RepairAutonomy,
) -> DiagSetup {
    let world = MiniWorld::new(config, files);
    DiagSetup {
        broker: ScriptedBroker::new(world.clone(), BrokerMode::Approve),
        world,
        policy: RepairPolicy {
            autonomy,
            ..RepairPolicy::default()
        },
    }
}

pub(super) fn consent() -> UserConsent {
    UserConsent {
        surface: "zdrowie-systemu".into(),
    }
}

/// Niskie ryzyko: wykryte, naprawione automatycznie, zweryfikowane, cofalne; brak pętli.
pub(super) async fn auto_repair_verify_and_undo<F, Fut>(factory: &F)
where
    F: Fn(DiagSetup) -> Fut,
    Fut: Future<Output = DiagHarness>,
{
    let s = setup(
        &[("voice.stt.device", json!("vulkan"))],
        &[],
        RepairAutonomy::AutoLowRisk,
    );
    let world = s.world.clone();
    let h = factory(s).await;
    h.diag.ingest(gpu_lost("voice.stt.device")).await;
    let out = h.diag.scan().await;
    assert_eq!((out.detected.len(), out.repaired.len()), (1, 1), "{out:?}");
    let after = world.snapshot();
    assert_eq!(after.config["voice.stt.device"], json!("cpu"));
    assert_eq!(after.restarts, ["voice-stt"]);
    // Ponowny skan na starych sygnałach — bez nowego incydentu.
    (h.advance)(1_000);
    assert!(h.diag.scan().await.detected.is_empty());
    let id = out.repaired[0];
    let rec = h.diag.undo(id).await.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(rec.status, RepairStatus::Undone);
    assert_eq!(world.snapshot().config["voice.stt.device"], json!("vulkan"));
    assert!(matches!(
        h.diag.undo(id).await,
        Err(DiagError::WrongStatus { .. })
    ));
    assert!(
        h.diag.scan().await.detected.is_empty(),
        "cofnięcie nie wraca w pętli"
    );
    assert!(!(h.events)().is_empty());
}

/// Średnie ryzyko czeka na zgodę; tryb „tylko propozycje” nic nie zmienia; limit napraw/h.
pub(super) async fn consent_and_autonomy<F, Fut>(factory: &F)
where
    F: Fn(DiagSetup) -> Fut,
    Fut: Future<Output = DiagHarness>,
{
    let s = setup(&[], &["modele/bielik.gguf"], RepairAutonomy::AutoLowRisk);
    let world = s.world.clone();
    let h = factory(s).await;
    let sha = "a".repeat(64);
    h.diag
        .ingest(symptom(
            "providers-local",
            Symptom::ModelHashMismatch,
            "modele/bielik.gguf",
            &[("sha256", &sha), ("model", "bielik")],
        ))
        .await;
    let out = h.diag.scan().await;
    assert_eq!(out.awaiting_consent.len(), 1, "{out:?}");
    let card = &h.diag.report().pending[0];
    assert!(!card.diff.is_empty() && !card.rationale.is_empty() && !card.rollback_plan.is_empty());
    assert_eq!((card.risk, card.consent), (Risk::Medium, Consent::User));
    assert!(world.snapshot().files.contains("modele/bielik.gguf"));
    let rec = h
        .diag
        .approve(out.awaiting_consent[0], consent())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(rec.status, RepairStatus::Verified);
    assert!(
        world
            .snapshot()
            .files
            .contains("kwarantanna/modele/bielik.gguf")
    );
    assert!(matches!(
        h.diag.approve(rec.id, consent()).await,
        Err(DiagError::WrongStatus { .. })
    ));

    let s = setup(
        &[("voice.stt.device", json!("vulkan"))],
        &[],
        RepairAutonomy::ProposeOnly,
    );
    let world = s.world.clone();
    let h = factory(s).await;
    h.diag.ingest(gpu_lost("voice.stt.device")).await;
    let out = h.diag.scan().await;
    assert_eq!(out.awaiting_consent.len(), 1);
    assert_eq!(world.snapshot().config["voice.stt.device"], json!("vulkan"));
    let rec = h
        .diag
        .reject(out.awaiting_consent[0])
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(rec.status, RepairStatus::Rejected);

    let mut s = setup(&[], &[], RepairAutonomy::AutoLowRisk);
    s.policy.max_auto_repairs_per_hour = 1;
    let h = factory(s).await;
    h.diag
        .ingest(symptom("router", Symptom::NetworkUnreachable, "x", &[]))
        .await;
    h.diag
        .ingest(symptom("router", Symptom::NetworkUnreachable, "x", &[]))
        .await;
    h.diag
        .ingest(symptom(
            "cost-meter",
            Symptom::BudgetExceeded,
            "miesiac",
            &[],
        ))
        .await;
    let out = h.diag.scan().await;
    assert_eq!(
        (out.repaired.len(), out.awaiting_consent.len()),
        (1, 1),
        "{out:?}"
    );
}

/// Nieudana weryfikacja i nieudany krok: wszystko cofnięte; wychładzanie i limit prób.
pub(super) async fn failures_roll_back<F, Fut>(factory: &F)
where
    F: Fn(DiagSetup) -> Fut,
    Fut: Future<Output = DiagHarness>,
{
    let s = setup(
        &[("voice.stt.device", json!("vulkan"))],
        &[],
        RepairAutonomy::AutoLowRisk,
    );
    let world = s.world.clone();
    world.set_verify(false);
    let h = factory(s).await;
    h.diag.ingest(gpu_lost("voice.stt.device")).await;
    let out = h.diag.scan().await;
    assert_eq!(out.failed.len(), 1, "{out:?}");
    assert_eq!(world.snapshot().config["voice.stt.device"], json!("vulkan"));
    h.diag.ingest(gpu_lost("voice.stt.device")).await;
    assert!(h.diag.scan().await.detected.is_empty(), "wychładzanie");

    let s = setup(
        &[("voice.stt.device", json!("vulkan"))],
        &[],
        RepairAutonomy::AutoLowRisk,
    );
    let world = s.world.clone();
    world.fail_restart("voice-stt");
    let h = factory(s).await;
    h.diag.ingest(gpu_lost("voice.stt.device")).await;
    let out = h.diag.scan().await;
    assert_eq!(out.failed.len(), 1);
    assert_eq!(
        world.snapshot().config["voice.stt.device"],
        json!("vulkan"),
        "krok 1 cofnięty"
    );
    let journal = h.diag.journal();
    assert!(journal.iter().any(
        |e| matches!(&e.event, JournalEvent::RolledBack { receipts, .. } if receipts.len() == 1)
    ));
}
