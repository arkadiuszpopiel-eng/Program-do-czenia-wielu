//! Zgody „zawsze w tym zakresie” po przeglądzie w świeżym kontekście 1.11: odwołanie bez Audytu
//! nadal kasuje plik, odwołanie unieważnia tokeny wydane na podstawie zgody, token nie żyje dłużej
//! niż zgoda, numery zgód rosną także po restarcie, STOP przy pliku zablokowanym do usunięcia nie
//! zostawia zgód na następne uruchomienie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::Path;
use std::sync::Arc;

use risk_classifier_contract::CommandOrigin;
use safety_broker_contract::contract_tests::{
    allowed, approve, delta, host, needs_approval, request,
};
use safety_broker_contract::{ApprovalDecision, Broker, Capability, UNTIL_REVOKED};
use safety_broker_impl::BrokerEngine;
use safety_broker_impl::audit::MemoryAudit;
use watchdog_contract::{Clock, ManualClock};
#[cfg(windows)]
use watchdog_contract::{KillReason, KillSwitch};

const HOUR: u64 = 3_600_000;

fn broker(path: &Path, clock: &Arc<ManualClock>, audit: Arc<MemoryAudit>) -> BrokerEngine {
    common::engine_with(
        safety_broker_contract::contract_tests::test_policy(),
        clock.clone(),
        audit,
    )
    .with_standing_level(path.to_path_buf())
}

fn ask(h: &str) -> safety_broker_contract::ActionRequest {
    request(
        &delta(),
        Capability::NetEgress(host(h)),
        CommandOrigin::UserText,
    )
}

async fn grant(b: &BrokerEngine, clock: &ManualClock, until_ms: u64) -> u64 {
    let ticket = needs_approval(b.decide(ask("a.example.org")).await);
    let scope = Capability::NetEgress(host("*.example.org"));
    approve(
        b,
        clock,
        ticket.id,
        ApprovalDecision::AllowInScope { scope, until_ms },
    )
    .await;
    b.grants().last().map(|g| g.id).unwrap()
}

#[tokio::test]
async fn revoke_without_audit_still_removes_the_saved_grant() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = broker(&path, &clock, audit.clone());
    let id = grant(&b, &clock, UNTIL_REVOKED).await;
    audit.set_failing(true);
    assert_eq!(
        b.revoke_grant(id).await,
        Ok(true),
        "odwołanie zawęża — bez Audytu też"
    );
    assert!(b.grants().is_empty());
    assert!(!dir.path().join("grants-standing.json").exists());
    drop(b);
    let b = broker(&path, &clock, Arc::new(MemoryAudit::default()));
    assert!(
        b.grants().is_empty(),
        "odwołana zgoda nie wraca po restarcie"
    );
}

#[tokio::test]
async fn revoking_a_grant_invalidates_tokens_issued_through_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let audit = Arc::new(MemoryAudit::default());
    let b = broker(&path, &clock, audit.clone());
    let id = grant(&b, &clock, clock.now_ms() + 3 * HOUR).await;
    let token = allowed(b.decide(ask("b.example.org")).await);
    let cap = Capability::NetEgress(host("b.example.org"));
    b.verify(&token, &cap, &delta()).unwrap();
    assert_eq!(b.revoke_grant(id).await, Ok(true));
    assert!(
        b.verify(&token, &cap, &delta()).is_err(),
        "token ze zgody traci ważność razem z nią"
    );
    assert!(audit.names().iter().any(|n| n == "broker.grant.revoked"));
}

#[tokio::test]
async fn token_from_a_grant_never_outlives_the_grant() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&path, &clock, Arc::new(MemoryAudit::default()));
    let until = clock.now_ms() + HOUR;
    grant(&b, &clock, until).await;
    let mut long = ask("b.example.org");
    long.ttl_ms = Some(4 * HOUR);
    let token = allowed(b.decide(long).await);
    assert!(
        token.expires_at_ms <= until,
        "token {} po końcu zgody {until}",
        token.expires_at_ms
    );
    clock.advance(HOUR);
    let cap = Capability::NetEgress(host("b.example.org"));
    assert!(
        b.verify(&token, &cap, &delta()).is_err(),
        "po końcu zgody token nie działa"
    );
}

#[tokio::test]
async fn grant_numbers_keep_growing_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&path, &clock, Arc::new(MemoryAudit::default()));
    let before = grant(&b, &clock, UNTIL_REVOKED).await;
    drop(b);
    clock.advance(60_000);
    let audit = Arc::new(MemoryAudit::default());
    let b = broker(&path, &clock, audit.clone());
    let restored = b.grants();
    assert_eq!(restored.len(), 1);
    assert!(
        restored[0].id > before,
        "stary numer nie trafi w inną zgodę"
    );
    assert!(audit.names().iter().any(|n| n == "broker.grant.restored"));
    assert_eq!(b.revoke_grant(before).await, Ok(false));
    // Przywrócona zgoda obejmuje `a.example.org` — odwołana, żeby udzielić nowej.
    assert_eq!(b.revoke_grant(restored[0].id).await, Ok(true));
    let fresh = grant(&b, &clock, clock.now_ms() + HOUR).await;
    assert!(fresh > restored[0].id);
}

/// Plik zgód otwarty przez inny program bez prawa usuwania (antywirus, indeksowanie): STOP
/// nadpisuje go pustą listą, więc po restarcie zgody nie wracają.
#[cfg(windows)]
#[tokio::test]
async fn stop_with_the_file_locked_against_delete_leaves_no_grants() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("autonomy-standing.json");
    let saved = dir.path().join("grants-standing.json");
    let clock = Arc::new(ManualClock::new(1_000_000));
    let b = broker(&path, &clock, Arc::new(MemoryAudit::default()));
    grant(&b, &clock, UNTIL_REVOKED).await;
    // FILE_SHARE_READ | FILE_SHARE_WRITE, bez FILE_SHARE_DELETE.
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x2)
        .open(&saved)
        .unwrap();
    assert!(
        std::fs::remove_file(&saved).is_err(),
        "plik naprawdę zablokowany"
    );
    b.kill_all(KillReason::Hotkey).await;
    assert!(b.grants().is_empty());
    drop(lock);
    drop(b);
    let b = broker(&path, &clock, Arc::new(MemoryAudit::default()));
    assert!(
        b.grants().is_empty(),
        "STOP przetrwał restart mimo blokady pliku"
    );
}
