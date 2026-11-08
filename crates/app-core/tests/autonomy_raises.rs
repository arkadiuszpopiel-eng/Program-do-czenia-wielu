//! Podniesienia poziomu autonomii zatwierdzone w oknie Brokera (zgłoszenia 2026-10-20 i 21):
//! strażnik je przyjmuje i ogłasza zamiast cofać do zapamiętanego obniżenia, ale tylko po zgodzie
//! potwierdzonej przez Brokera dla tej prośby. Prośba bez zgody — odrzucona, bez decyzji albo
//! nieznana nowemu Brokerowi z poziomem domyślnym L3 — nie nadpisuje obniżenia właściciela
//! (przegląd 2026-10-21).
//!
//! Strażnik porzuconego rdzenia działa dalej (zadanie w tle), dlatego każde ponowne uruchomienie
//! aplikacji dostaje nowy obiekt Brokera, a asercje „nic nie wysłano” czekają na przebieg strażnika.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use app_core::dto::{AutonomyLevel, BrokerIntentStatus, SessionTemplate};
use app_core::ports::ApprovalOutcome;
use app_core::{AppCore, MemorySecretStore};
use common::broker::{Broker, announced, eventually, passes, start};

/// Prośba o podniesienie, która otwiera kartę w oknie Brokera; zwraca identyfikator prośby.
async fn ask(core: &AppCore, level: AutonomyLevel, session: Option<&String>) -> String {
    let asked = core
        .permissions_request_level(level, session.cloned())
        .await
        .unwrap();
    assert_eq!(asked.status, BrokerIntentStatus::OpenedBroker);
    asked.request_id
}

/// Czeka na ogłoszenie po przyjęciu zgody (strona Uprawnień się odświeża).
async fn announced_after(core: &AppCore, before: usize) {
    assert!(
        eventually(|| async { announced(core).await > before }).await,
        "brak ogłoszenia po przyjęciu zgody (przed: {before})"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raise_approved_in_broker_window_is_adopted_announced_and_kept() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let broker = Broker::with_window(AutonomyLevel::L3);
    let core = start(dir.path(), broker.clone(), &secrets).await;
    let session = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    // Obniżenia (zapamiętane): globalne do L2, sesji do L1; potem L4 globalnie i dla sesji — dwie
    // karty w oknie Brokera.
    core.permissions_request_level(AutonomyLevel::L2, None)
        .await
        .unwrap();
    core.permissions_request_level(AutonomyLevel::L1, Some(session.clone()))
        .await
        .unwrap();
    let global = ask(&core, AutonomyLevel::L4, None).await;
    let own = ask(&core, AutonomyLevel::L4, Some(&session)).await;
    let before = announced(&core).await;
    broker.requests.lock().unwrap().clear();
    broker.approve(&global, AutonomyLevel::L4, None);
    broker.approve(&own, AutonomyLevel::L4, Some(session.as_str()));
    announced_after(&core, before).await;
    // Kolejny przebieg strażnika nie cofa zgody do zapamiętanych L2 i L1.
    passes(&broker, 2).await;
    assert!(broker.sent().is_empty(), "{:?}", broker.sent());
    assert_eq!(*broker.global.lock().unwrap(), AutonomyLevel::L4);
    assert_eq!(
        broker.sessions.lock().unwrap().get(&session).copied(),
        Some(AutonomyLevel::L4)
    );
    drop(core);

    // Ponowne uruchomienie aplikacji, Broker (usługa) zostaje na L4: L4 jest zapamiętane.
    let service = Broker::with_window(AutonomyLevel::L4);
    service
        .sessions
        .lock()
        .unwrap()
        .insert(session.clone(), AutonomyLevel::L4);
    let core = start(dir.path(), service.clone(), &secrets).await;
    passes(&service, 2).await;
    assert!(service.sent().is_empty(), "{:?}", service.sent());
    // Obniżenia nadal działają od razu.
    core.permissions_request_level(AutonomyLevel::L1, None)
        .await
        .unwrap();
    assert_eq!(*service.global.lock().unwrap(), AutonomyLevel::L1);
    drop(core);
}

/// Zgłoszenie 21.10: nowy profil (nic wcześniej nie obniżono), L4 zatwierdzone w oknie Brokera —
/// strażnik odnotowuje je i ogłasza (wcześniej przechodziło bez śladu, strona pokazywała stary poziom).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raise_approved_on_a_new_profile_is_adopted_and_announced() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let broker = Broker::with_window(AutonomyLevel::L3);
    let core = start(dir.path(), broker.clone(), &secrets).await;
    let _session = core.sessions_create(SessionTemplate::Empty).await.unwrap();
    let global = ask(&core, AutonomyLevel::L4, None).await;
    let before = announced(&core).await;
    broker.approve(&global, AutonomyLevel::L4, None);
    announced_after(&core, before).await;
    drop(core);
    // Zapamiętane L4: Broker usługi z L4 po ponownym uruchomieniu aplikacji niczego nie dostaje.
    let service = Broker::with_window(AutonomyLevel::L4);
    let core = start(dir.path(), service.clone(), &secrets).await;
    passes(&service, 2).await;
    assert!(service.sent().is_empty(), "{:?}", service.sent());
    drop(core);
}

/// Dwie karty dla tego samego zakresu: zgoda na pierwszą jest przyjęta (druga prośba jej nie
/// zastępuje) — zapamiętany poziom to L2, nie wcześniejsze obniżenie L1.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn approving_the_first_of_two_cards_is_adopted() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let broker = Broker::with_window(AutonomyLevel::L3);
    let core = start(dir.path(), broker.clone(), &secrets).await;
    let _session = core.sessions_create(SessionTemplate::Empty).await.unwrap();
    core.permissions_request_level(AutonomyLevel::L1, None)
        .await
        .unwrap();
    let first = ask(&core, AutonomyLevel::L2, None).await;
    let second = ask(&core, AutonomyLevel::L4, None).await;
    let before = announced(&core).await;
    broker.requests.lock().unwrap().clear();
    broker.approve(&first, AutonomyLevel::L2, None);
    announced_after(&core, before).await;
    broker
        .approvals
        .lock()
        .unwrap()
        .insert(second, ApprovalOutcome::Rejected);
    passes(&broker, 2).await;
    assert!(broker.sent().is_empty(), "{:?}", broker.sent());
    assert_eq!(*broker.global.lock().unwrap(), AutonomyLevel::L2);
    drop(core);
    // Nowy Broker z poziomem domyślnym: strażnik nakłada zapamiętane L2.
    let fresh = Broker::with_window(AutonomyLevel::L3);
    let core = start(dir.path(), fresh.clone(), &secrets).await;
    assert!(
        eventually(|| async { fresh.sent().contains(&(AutonomyLevel::L2, None)) }).await,
        "{:?}",
        fresh.sent()
    );
    drop(core);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raise_without_approval_does_not_override_the_owners_lowering() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecretStore::default());
    let broker = Broker::with_window(AutonomyLevel::L3);
    let core = start(dir.path(), broker.clone(), &secrets).await;
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
    let lowered = |broker: &Broker| {
        let sent = broker.sent();
        sent.contains(&(AutonomyLevel::L1, None))
            && sent.contains(&(AutonomyLevel::L0, Some(session.clone())))
    };

    // Prośby o L3 (poziom domyślny Brokera): globalna bez decyzji, sesji odrzucona.
    let _global = ask(&core, AutonomyLevel::L3, None).await;
    let own = ask(&core, AutonomyLevel::L3, Some(&session)).await;
    broker
        .approvals
        .lock()
        .unwrap()
        .insert(own, ApprovalOutcome::Rejected);
    // 1. Broker uruchomiony od nowa w trakcie pracy: ma L3 jak prośby, ale ich nie zna.
    broker.requests.lock().unwrap().clear();
    broker.restart();
    assert!(
        eventually(|| async { lowered(&broker) }).await,
        "{:?}",
        broker.sent()
    );
    assert_eq!(*broker.global.lock().unwrap(), AutonomyLevel::L1);

    // 2. Nowa prośba bez decyzji, potem ponowne uruchomienie aplikacji z nowym Brokerem (tryb
    // przenośny: Broker kończy się z aplikacją) w ciągu 15 minut.
    let _again = ask(&core, AutonomyLevel::L3, None).await;
    drop(core);
    let fresh = Broker::with_window(AutonomyLevel::L3);
    let core = start(dir.path(), fresh.clone(), &secrets).await;
    assert!(
        eventually(|| async { lowered(&fresh) }).await,
        "{:?}",
        fresh.sent()
    );
    assert_eq!(*fresh.global.lock().unwrap(), AutonomyLevel::L1);
    assert_eq!(
        fresh.sessions.lock().unwrap().get(&session).copied(),
        Some(AutonomyLevel::L0)
    );
    drop(core);
}
