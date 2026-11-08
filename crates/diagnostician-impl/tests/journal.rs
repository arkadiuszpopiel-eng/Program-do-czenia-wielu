//! Dziennik napraw w pliku: trwałość po restarcie, odzysk naprawy przerwanej między wykonaniem
//! a weryfikacją (cofnięcie), uszkodzona ostatnia linia nie blokuje startu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::Write as _;
use std::sync::Arc;

use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use diagnostician_contract::{
    Diagnostician, JournalEntry, JournalEvent, RepairAutonomy, RepairPolicy, RepairStatus,
};
use diagnostician_fake::{ChaosBroker, ChaosWorld, inject};
use watchdog_contract::ManualClock;

fn world() -> (Arc<ManualClock>, Arc<ChaosWorld>) {
    let clock = Arc::new(ManualClock::new(1_700_000_000_000));
    let world = ChaosWorld::baseline(Arc::clone(&clock));
    (clock, world)
}

#[tokio::test]
async fn repairs_survive_restart_and_stay_undoable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("diagnostyka/naprawy.ndjson");
    let (clock, world) = world();
    let faulted = {
        let signals = inject(&world, "f07-gpu-lost").unwrap();
        let snap = world.snapshot();
        let svc = common::service(
            world.clone(),
            world.clone(),
            Arc::new(ChaosBroker(world.clone())),
            RepairPolicy::default(),
            clock.clone(),
            Some(path.clone()),
        )
        .await;
        for s in signals {
            svc.ingest(s).await;
        }
        assert_eq!(svc.scan().await.repaired.len(), 1);
        snap
    };
    let svc = common::service(
        world.clone(),
        world.clone(),
        Arc::new(ChaosBroker(world.clone())),
        RepairPolicy::default(),
        clock,
        Some(path.clone()),
    )
    .await;
    let repairs = svc.repairs();
    assert_eq!(repairs.len(), 1);
    assert_eq!(repairs[0].status, RepairStatus::Verified);
    assert!(svc.journal_problems().is_empty());
    assert_eq!(svc.journal().len(), 3);
    svc.undo(repairs[0].id).await.unwrap();
    assert_eq!(world.snapshot(), faulted);
    let lines = std::fs::read_to_string(&path).unwrap().lines().count();
    assert_eq!(lines, 4, "dziennik dopisywany, nie nadpisywany");
}

#[tokio::test]
async fn interrupted_repair_is_rolled_back_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("naprawy.ndjson");
    let (clock, world) = world();
    let signals = inject(&world, "f20-network-down").unwrap();
    let faulted = world.snapshot();
    let policy = RepairPolicy {
        autonomy: RepairAutonomy::ProposeOnly,
        ..RepairPolicy::default()
    };
    let id = {
        let svc = common::service(
            world.clone(),
            world.clone(),
            Arc::new(ChaosBroker(world.clone())),
            policy.clone(),
            clock.clone(),
            Some(path.clone()),
        )
        .await;
        for s in signals {
            svc.ingest(s).await;
        }
        svc.scan().await.awaiting_consent[0]
    };
    // Symulacja awarii procesu: kroki wykonane, wpis `applied` zapisany, weryfikacji brak.
    let entries: Vec<JournalEntry> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let JournalEvent::Proposed { proposal, .. } = &entries[0].event else {
        panic!("brak propozycji")
    };
    let mut receipts = Vec::new();
    for step in &proposal.steps {
        receipts.push(
            diagnostician_contract::RepairEnv::apply(world.as_ref(), step)
                .await
                .unwrap(),
        );
    }
    assert_ne!(world.snapshot(), faulted);
    let applied = JournalEntry {
        seq: 2,
        ts_ms: entries[0].ts_ms + 1,
        repair: id,
        kind: entries[0].kind,
        target: entries[0].target.clone(),
        event: JournalEvent::Applied { receipts },
    };
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(f, "{}", serde_json::to_string(&applied).unwrap()).unwrap();
    writeln!(f, "{{\"seq\": 3, \"niedokończ").unwrap();
    drop(f);
    let mut svc = common::service(
        world.clone(),
        world.clone(),
        Arc::new(ChaosBroker(world.clone())),
        policy,
        clock,
        Some(path),
    )
    .await;
    assert_eq!(
        world.snapshot(),
        faulted,
        "przerwana naprawa cofnięta przy otwarciu"
    );
    assert!(matches!(
        svc.repairs()[0].status,
        RepairStatus::Failed { .. }
    ));
    assert_eq!(svc.journal_problems().len(), 1);
    let bus = FakeBus::default();
    svc.start(ModuleContext::new(svc.manifest().id.clone(), Arc::new(bus)))
        .await
        .unwrap();
    assert!(matches!(svc.health(), HealthStatus::Degraded(_)));
    svc.stop().await.unwrap();
}
