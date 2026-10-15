//! Polecenia bez modelu (P5) na prawdziwym rdzeniu: rachunek i data dostają odpowiedź bez
//! wywołania dostawcy (bez kosztu i tokenów), działają offline zamiast trafiać do kolejki,
//! zwykłe pytanie idzie do modelu, a wyłączone ustawienie przywraca drogę przez model.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use app_core::dto::{SessionTemplate, SettingValue, Turn, TurnStatus};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::*;

async fn reply(core: &AppCore, sid: &str, text: &str) -> (Turn, Option<String>) {
    let sent = core
        .turns_send(sid.to_owned(), send(text, None))
        .await
        .unwrap();
    let id = sent.assistant_turn_id.clone();
    for _ in 0..300 {
        let turns = core.turns_list(sid.to_owned()).await.unwrap().turns;
        if let Some(t) = turns
            .iter()
            .find(|t| Some(&t.id) == id.as_ref() && t.status != TurnStatus::Streaming)
        {
            return (t.clone(), id);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("brak odpowiedzi na: {text}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_commands_answer_without_a_model() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let opts = options(Some(provider.clone()), Arc::new(HeadlessShell::default()));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;

    let (math, _) = reply(&core, &sid, "Ile to jest 7 razy 8?").await;
    assert_eq!(math.status, TurnStatus::Complete);
    assert_eq!(math.text, "7 × 8 = **56**");
    assert!(math.usage.is_none(), "bez tokenów i kosztu");
    let (date, _) = reply(&core, &sid, "Jaki dziś dzień?").await;
    assert!(date.text.starts_with("Dziś jest "), "{}", date.text);
    assert!(provider.requests().is_empty(), "model nie był wywołany");

    // Offline: polecenie dostaje odpowiedź od razu, zwykła wiadomość czeka w kolejce.
    core.set_online(false).await;
    let (spell, id) = reply(&core, &sid, "Przeliteruj rower").await;
    assert!(id.is_some(), "polecenie offline nie trafia do kolejki");
    assert!(spell.text.contains("R – O – W – E – R"), "{}", spell.text);
    let queued = core
        .turns_send(sid.clone(), send("Opowiedz mi o Krakowie", None))
        .await
        .unwrap();
    assert_eq!(queued.assistant_turn_id, None, "zwykła wiadomość w kolejce");
    core.set_online(true).await;
    core.system_retry_queue().await.unwrap();
    for _ in 0..300 {
        if !provider.requests().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        provider.requests().len(),
        1,
        "zwykłe pytanie idzie do modelu"
    );
    core.turns_stop(sid.clone()).await.unwrap();

    // Wyłączone ustawienie: rachunek idzie do modelu.
    core.settings_set("sessions.local_commands".into(), SettingValue::Bool(false))
        .await
        .unwrap();
    let before = provider.requests().len();
    core.turns_send(sid.clone(), send("2+2", None))
        .await
        .unwrap();
    for _ in 0..300 {
        if provider.requests().len() > before {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        provider.requests().len(),
        before + 1,
        "wyłączone — przez model"
    );
    core.turns_stop(sid).await.unwrap();
}
