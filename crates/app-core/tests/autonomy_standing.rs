//! „Zezwól i przełącz na Automatycznie” w oknie Sędziego (plan 50 §4.2): Sędzia sam podnosi tryb
//! globalny i go zapamiętuje. Strażnik rdzenia przyjmuje poziom zapamiętany przez Sędziego zamiast
//! cofać go do obniżenia właściciela; bez zapisu w Sędzim (np. Broker po restarcie bez pliku)
//! obniżenie wraca jak dotąd.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use app_core::MemorySecretStore;
use app_core::dto::AutonomyLevel;
use common::broker::{Broker, passes, start};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn level_remembered_by_the_judge_is_adopted_not_reverted() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let broker = Broker::with_window(AutonomyLevel::L3);
    let core = start(dir.path(), broker.clone(), &secrets).await;
    core.permissions_request_level(AutonomyLevel::L1, None)
        .await
        .unwrap();
    assert_eq!(*broker.global.lock().unwrap(), AutonomyLevel::L1);

    // Właściciel w oknie Sędziego: „Zezwól i przełącz na Automatycznie”.
    *broker.global.lock().unwrap() = AutonomyLevel::L3;
    *broker.standing.lock().unwrap() = Some(AutonomyLevel::L3);
    broker.requests.lock().unwrap().clear();
    passes(&broker, 2).await;
    assert!(broker.sent().is_empty(), "{:?}", broker.sent());
    assert_eq!(*broker.global.lock().unwrap(), AutonomyLevel::L3);
    assert_eq!(
        core.permissions_get(None).await.unwrap().global,
        AutonomyLevel::L3
    );
    drop(core);

    // Restart aplikacji, Broker bez zapisu na poziomie domyślnym: zostaje przyjęte L3 (nie L1).
    let fresh = Broker::with_window(AutonomyLevel::L3);
    let core = start(dir.path(), fresh.clone(), &secrets).await;
    passes(&fresh, 2).await;
    assert!(fresh.sent().is_empty(), "{:?}", fresh.sent());
    drop(core);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn without_the_judge_record_the_owner_lowering_returns() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let broker = Broker::with_window(AutonomyLevel::L3);
    let core = start(dir.path(), broker.clone(), &secrets).await;
    core.permissions_request_level(AutonomyLevel::L1, None)
        .await
        .unwrap();
    // Broker po restarcie bez pliku: L3 bez zapisu — strażnik przywraca obniżenie.
    *broker.global.lock().unwrap() = AutonomyLevel::L3;
    broker.requests.lock().unwrap().clear();
    passes(&broker, 2).await;
    assert_eq!(*broker.global.lock().unwrap(), AutonomyLevel::L1);
    drop(core);
}
