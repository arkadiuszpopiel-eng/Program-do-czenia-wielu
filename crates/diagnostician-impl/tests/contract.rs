//! Implementacja: testy kontraktowe, testy chaosowe (katalog F8-01, karty F8-06) i zgodność
//! katalogu z zestawem `evals/F8/chaos/catalog.json`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use diagnostician_contract::contract_tests::{self, DiagHarness, DiagSetup};
use diagnostician_contract::{Consent, Diagnostician, FailureKind};
use diagnostician_fake::{ChaosBroker, FAULTS, run_chaos};
use serde::Deserialize;
use watchdog_contract::ManualClock;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|s: DiagSetup| async move {
        let dir = tempfile::tempdir().unwrap();
        let clock = Arc::new(ManualClock::new(1_000));
        let svc = common::service(
            s.world.clone(),
            s.world,
            s.broker,
            s.policy,
            clock.clone(),
            Some(dir.path().join("naprawy.ndjson")),
        )
        .await;
        let svc = Arc::new(svc);
        let events = Arc::clone(&svc);
        // Katalog tymczasowy żyje tyle, co uprząż (zamknięcie go trzyma).
        let keep = Arc::new(dir);
        DiagHarness {
            diag: svc as Arc<dyn Diagnostician>,
            advance: Arc::new(move |ms| {
                let _ = &keep;
                clock.advance(ms);
            }),
            events: Arc::new(move || events.recent_events()),
        }
    })
    .await;
}

#[tokio::test]
async fn chaos_catalog_on_impl() {
    let report = run_chaos(
        |s| async move {
            let broker = Arc::new(ChaosBroker(s.world.clone()));
            let svc =
                common::service(s.world.clone(), s.world, broker, s.policy, s.clock, None).await;
            Arc::new(svc) as Arc<dyn Diagnostician>
        },
        &[],
    )
    .await;
    eprintln!("F8-01 (implementacja):\n{}", report.to_markdown());
    let bad: Vec<_> = report.results.iter().filter(|r| !r.ok()).collect();
    assert!(bad.is_empty(), "{bad:#?}");
    assert!(report.all_ok() && report.results.len() >= 20);
}

#[derive(Deserialize)]
struct Catalog {
    faults: Vec<CatalogFault>,
}

#[derive(Deserialize)]
struct CatalogFault {
    id: String,
    kind: FailureKind,
    target: String,
    consent: Consent,
}

#[test]
fn evals_catalog_matches_chaos_world() {
    let catalog: Catalog =
        serde_json::from_str(include_str!("../../../evals/F8/chaos/catalog.json")).unwrap();
    assert!(catalog.faults.len() >= 20);
    assert_eq!(catalog.faults.len(), FAULTS.len());
    for (c, f) in catalog.faults.iter().zip(FAULTS.iter()) {
        assert_eq!(
            (c.id.as_str(), c.kind, c.target.as_str()),
            (f.id, f.kind, f.target)
        );
        assert_eq!(
            c.consent == Consent::Broker,
            f.kind.kernel_only(),
            "{}",
            c.id
        );
    }
    let kinds: std::collections::BTreeSet<_> = catalog.faults.iter().map(|c| c.kind).collect();
    assert_eq!(
        kinds.len(),
        FailureKind::ALL.len(),
        "każdy rodzaj katalogu ma awarię chaosową"
    );
}
