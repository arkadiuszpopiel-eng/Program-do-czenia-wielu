//! Kill-switch: od wywołania do zakończenia (tokeny + Job Objects + cisza audio + Audyt)
//! < 200 ms p95 z 50 prób na atrapach (ACC-F3-safety-broker-01 dotyczy prawdziwego systemu —
//! tu mierzymy logikę). Ściśle przy `ALFA_PERF_BUDGETS=1`, inaczej próg ×10 (crates/README.md).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Instant;

use platform_contract::ProcessHandle;
use safety_broker_contract::contract_tests::{delta, request, tree};
use safety_broker_contract::{Broker, Capability, CommandOrigin, Decision};
use watchdog_contract::{JobRegistry, KillReason, KillSwitch, ProcessRole};

fn budget_ms(strict_ms: u128) -> u128 {
    if std::env::var_os("ALFA_PERF_BUDGETS").is_some() {
        strict_ms
    } else {
        strict_ms * 10
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_switch_p95_under_200ms() {
    let mut samples = Vec::with_capacity(50);
    for trial in 0..50u32 {
        let (b, audit, _) = common::engine();
        let mut tokens = Vec::new();
        for i in 0..100 {
            let cap = Capability::FsRead(tree(&format!(r"C:\Users\ala\Docs\d{i}")));
            let Ok(Decision::Allow(t)) = b
                .decide(request(&delta(), cap, CommandOrigin::UserText))
                .await
            else {
                panic!("brak tokenu")
            };
            tokens.push(t);
        }
        for j in 0..20 {
            b.register_job(
                ProcessHandle(trial * 100 + j),
                ProcessRole::Tool(format!("t{j}")),
                "x",
            );
        }
        let started = Instant::now();
        let report = b.kill_all(KillReason::Hotkey).await;
        samples.push(started.elapsed().as_micros());
        assert_eq!(report.tokens_revoked, 100);
        assert_eq!(report.jobs_killed, 20);
        assert!(report.audio_silenced && report.audited);
        assert!(audit.names().iter().any(|n| n == "broker.kill_switch"));
        let probe = Capability::FsRead(tree(r"C:\Users\ala\Docs\d0"));
        assert!(
            tokens
                .iter()
                .all(|t| b.verify(t, &probe, &delta()).is_err())
        );
    }
    samples.sort_unstable();
    let p95 = samples[(samples.len() * 95).div_ceil(100) - 1];
    let max = samples[samples.len() - 1];
    eprintln!(
        "kill-switch (logika, 50 prób): p95 = {p95} µs, max = {max} µs, budżet {} ms",
        budget_ms(200)
    );
    assert!(p95 < budget_ms(200) * 1000, "p95 {p95} µs");
}
