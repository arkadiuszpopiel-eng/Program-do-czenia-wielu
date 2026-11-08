//! Zapamiętane obniżenia autonomii a Broker startujący później albo ponownie (audyt 2026-10-14,
//! A-04): obniżenie globalne i obniżenie sesji wracają po ponownym uruchomieniu aplikacji, także
//! gdy Broker odpowiada dopiero po chwili z poziomem domyślnym. Podniesienia — `autonomy_raises.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use app_core::MemorySecretStore;
use app_core::dto::{AutonomyLevel, SessionTemplate};
use common::broker::{Broker, passes, start};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lowered_levels_come_back_when_a_late_broker_starts_with_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());

    // Pierwsze uruchomienie: właściciel obniża poziom globalny do L1, a jednej sesji do L0.
    let first = Arc::new(Broker::default());
    first.ready.store(true, Ordering::SeqCst);
    let core = start(dir.path(), first.clone(), &secrets).await;
    let session = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    core.permissions_request_level(AutonomyLevel::L1, None)
        .await
        .unwrap();
    core.permissions_request_level(AutonomyLevel::L0, Some(session.clone()))
        .await
        .unwrap();
    drop(core);

    // Drugie uruchomienie: Broker odpowiada dopiero po chwili, z poziomem domyślnym L3.
    let late = Arc::new(Broker::default());
    let core = start(dir.path(), late.clone(), &secrets).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        late.requests.lock().unwrap().is_empty(),
        "Sędzia Cieni nieosiągalny — nic nie wysłano"
    );
    late.ready.store(true, Ordering::SeqCst);
    let mut waited = Duration::ZERO;
    while late.requests.lock().unwrap().len() < 2 && waited < Duration::from_secs(10) {
        tokio::time::sleep(Duration::from_millis(100)).await;
        waited += Duration::from_millis(100);
    }
    let requests = late.requests.lock().unwrap().clone();
    assert!(
        requests.contains(&(AutonomyLevel::L1, None)),
        "{requests:?}"
    );
    assert!(
        requests.contains(&(AutonomyLevel::L0, Some(session.clone()))),
        "{requests:?}"
    );
    assert_eq!(*late.global.lock().unwrap(), AutonomyLevel::L1);

    // Kolejne sprawdzenie nic nie zmienia — Broker ma już poziomy niższe albo równe.
    late.requests.lock().unwrap().clear();
    passes(&late, 1).await;
    assert!(late.requests.lock().unwrap().is_empty());
    drop(core);
}
