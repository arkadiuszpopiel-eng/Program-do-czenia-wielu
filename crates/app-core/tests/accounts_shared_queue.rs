//! Wspólna kolejka modeli wszystkich kont (zgłoszenie właściciela 2026-10-14: kolejkę da się ustawić
//! tylko osobno dla każdego konta — potrzebna jeszcze jedna, wspólna). Ścieżka przez prawdziwy
//! adapter i Router do lokalnego serwera udającego dostawcę, jak w `accounts_models.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::dto::{SessionTemplate, SharedQueueEntry};
use common::chat_stub::{MISSING, chat_stub};
use common::keys::{core, input};
use common::{ends, send, until};

const KEY: &str = "test-wspolna-XXXXXXXXXXXXXXXXXXXX";

fn entry(account: &str, model: &str) -> SharedQueueEntry {
    SharedQueueEntry {
        account_id: account.to_owned(),
        model: model.to_owned(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_queue_goes_before_account_queues_and_clearing_it_restores_them() {
    let (base, asked) = chat_stub();
    let (core, _dir) = core().await;
    let mut rx = core.subscribe_events();
    let account = core.accounts_add(input(KEY, &base, "praca")).await.unwrap();
    assert!(core.accounts_test(account.id.clone()).await.unwrap().ok);
    let session = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let ask = |text: &'static str| {
        let (core, session) = (core.clone(), session.clone());
        async move {
            core.turns_send(session, send(text, None))
                .await
                .unwrap()
                .assistant_turn_id
                .unwrap()
        }
    };

    // Kolejka konta: gpt-4o. Bez wspólnej kolejki — ona decyduje.
    core.accounts_set_queue(account.id.clone(), "chat".into(), vec!["gpt-4o".into()])
        .await
        .unwrap();
    assert!(core.accounts_shared_queues().await.unwrap().is_empty());
    let turn = ask("Pierwsze").await;
    until(&mut rx, ends(&turn)).await;
    assert_eq!(*asked.lock().unwrap(), ["gpt-4o"]);

    // Wspólna kolejka ma pierwszeństwo; model, którego konto nie ma, jest pomijany.
    let saved = core
        .accounts_set_shared_queue(
            "chat".into(),
            vec![
                entry(&account.id, MISSING),
                entry(&account.id, "gpt-4o-mini"),
            ],
        )
        .await
        .unwrap();
    assert_eq!(saved["chat"].len(), 2);
    assert_eq!(core.accounts_shared_queues().await.unwrap(), saved);
    asked.lock().unwrap().clear();
    let turn = ask("Drugie").await;
    until(&mut rx, ends(&turn)).await;
    assert_eq!(*asked.lock().unwrap(), [MISSING, "gpt-4o-mini"]);

    // Pusta lista usuwa wspólną kolejkę klasy — wraca kolejka konta.
    let cleared = core
        .accounts_set_shared_queue("chat".into(), Vec::new())
        .await
        .unwrap();
    assert!(!cleared.contains_key("chat"));
    asked.lock().unwrap().clear();
    let turn = ask("Trzecie").await;
    until(&mut rx, ends(&turn)).await;
    assert_eq!(*asked.lock().unwrap(), ["gpt-4o"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_queue_refuses_bad_entries_with_a_clear_message() {
    let (base, _asked) = chat_stub();
    let (core, _dir) = core().await;
    let account = core.accounts_add(input(KEY, &base, "praca")).await.unwrap();
    let refused = |result: Result<_, app_core::AppError>| result.unwrap_err().message;

    let unknown_class = core
        .accounts_set_shared_queue("rozmowa".into(), vec![entry(&account.id, "gpt-4o")])
        .await;
    assert!(refused(unknown_class).contains("klasa zadań"));
    let unknown_account = core
        .accounts_set_shared_queue("chat".into(), vec![entry("acc-nie-ma", "gpt-4o")])
        .await;
    assert!(refused(unknown_account).contains("nie istnieje"));
    let twice = core
        .accounts_set_shared_queue(
            "chat".into(),
            vec![entry(&account.id, "gpt-4o"), entry(&account.id, "gpt-4o")],
        )
        .await;
    assert!(refused(twice).contains("dwa razy"));
    let empty_model = core
        .accounts_set_shared_queue("chat".into(), vec![entry(&account.id, "  ")])
        .await;
    assert!(refused(empty_model).contains("pusta"));
    let too_many = core
        .accounts_set_shared_queue(
            "chat".into(),
            (0..11)
                .map(|i| entry(&account.id, &format!("model-{i}")))
                .collect(),
        )
        .await;
    assert!(refused(too_many).contains("najwyżej 10"));
    assert!(core.accounts_shared_queues().await.unwrap().is_empty());
}
