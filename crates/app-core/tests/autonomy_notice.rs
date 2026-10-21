//! Przegląd 2026-10-21: po ponownym uruchomieniu (nowy Broker z poziomem domyślnym L3) zapamiętane
//! L4 nie wraca samo — wymaga zgody w oknie Brokera. Wcześniej poziom spadał bez słowa, a zadania
//! w tle dostawały odmowy; teraz właściciel dostaje jedną informację na uruchomienie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use app_core::dto::{AlfaEvent, AutonomyLevel, SessionTemplate};
use app_core::{AppCore, MemorySecretStore};
use common::broker::{Broker, announced, eventually, passes, start};

async fn notices(core: &AppCore) -> usize {
    core.events_resume(0, None)
        .await
        .unwrap()
        .frames
        .iter()
        .flat_map(|f| f.events.iter())
        .filter(|e| {
            matches!(e, AlfaEvent::Toast { message, .. } if message.pl.contains("wymaga ponownego potwierdzenia"))
        })
        .count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remembered_raise_missing_after_restart_is_announced_once() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let broker = Broker::with_window(AutonomyLevel::L3);
    let core = start(dir.path(), broker.clone(), &secrets).await;
    // Przyjęcie zgody ogłasza się zmianą sesji — potrzebna choć jedna.
    core.sessions_create(SessionTemplate::Empty).await.unwrap();
    let asked = core
        .permissions_request_level(AutonomyLevel::L4, None)
        .await
        .unwrap();
    let before = announced(&core).await;
    broker.approve(&asked.request_id, AutonomyLevel::L4, None);
    assert!(eventually(|| async { announced(&core).await > before }).await);
    passes(&broker, 1).await;
    assert_eq!(notices(&core).await, 0, "Broker ma zapamiętany poziom");
    drop(core);

    // Restart: nowy Broker na L3, zapamiętane L4 — jedna informacja mimo kilku przebiegów.
    let fresh = Broker::with_window(AutonomyLevel::L3);
    let core = start(dir.path(), fresh.clone(), &secrets).await;
    assert!(eventually(|| async { notices(&core).await == 1 }).await);
    passes(&fresh, 2).await;
    assert_eq!(notices(&core).await, 1);
    assert_eq!(
        *fresh.global.lock().unwrap(),
        AutonomyLevel::L3,
        "bez podniesienia"
    );
}
