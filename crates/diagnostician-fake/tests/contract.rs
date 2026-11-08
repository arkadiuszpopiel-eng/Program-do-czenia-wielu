//! Atrapa: testy kontraktowe i testy chaosowe (cały katalog awarii).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use diagnostician_contract::Diagnostician;
use diagnostician_contract::contract_tests::{self, DiagHarness, DiagSetup};
use diagnostician_fake::{FAULTS, chaos_diagnostician, fake_diagnostician, run_chaos};
use watchdog_contract::ManualClock;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|s: DiagSetup| async move {
        let clock = Arc::new(ManualClock::new(1_000));
        let (host, core) =
            fake_diagnostician(clock.clone(), s.world.clone(), s.world, s.broker, s.policy);
        DiagHarness {
            diag: Arc::new(core) as Arc<dyn Diagnostician>,
            advance: Arc::new(move |ms| clock.advance(ms)),
            events: Arc::new(move || host.events()),
        }
    })
    .await;
}

#[tokio::test]
async fn chaos_catalog_on_fake() {
    let report = run_chaos(
        |s| async move { Arc::new(chaos_diagnostician(s).1) as Arc<dyn Diagnostician> },
        &[],
    )
    .await;
    eprintln!("{}", report.to_markdown());
    let bad: Vec<_> = report.results.iter().filter(|r| !r.ok()).collect();
    assert!(bad.is_empty(), "{bad:#?}");
    assert_eq!(report.results.len(), FAULTS.len());
    assert!(report.results.len() >= 20);
}
