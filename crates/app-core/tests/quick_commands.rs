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

/// Liczby z ostatniego pytania quizu („**a × b = ?**”).
fn question(text: &str) -> (u64, u64) {
    let q = text.rsplit("**").nth(1).unwrap();
    let (a, rest) = q.split_once(" × ").unwrap();
    let b = rest.trim_end_matches(" = ?");
    (a.parse().unwrap(), b.parse().unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multiplication_quiz_keeps_state_per_session() {
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
    let (first, _) = reply(&core, &sid, "Przepytaj mnie z tabliczki mnożenia przez 7").await;
    let (a, b) = question(&first.text);
    assert_eq!(a, 7, "{}", first.text);
    let (good, _) = reply(&core, &sid, &(a * b).to_string()).await;
    assert!(good.text.starts_with("Dobrze!"), "{}", good.text);
    let (c, d) = question(&good.text);
    let (bad, _) = reply(&core, &sid, &(c * d + 1).to_string()).await;
    assert!(bad.text.starts_with("Prawie"), "{}", bad.text);
    let (end, _) = reply(&core, &sid, "koniec").await;
    assert!(end.text.contains("Dobrze: 1 z 2"), "{}", end.text);
    // Po zakończeniu liczba to zwykła wiadomość — idzie do modelu.
    core.turns_send(sid.clone(), send("42", None))
        .await
        .unwrap();
    for _ in 0..300 {
        if !provider.requests().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(provider.requests().len(), 1);
    core.turns_stop(sid).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_commands_reply_test_and_validate() {
    use app_core::dto::{CustomCommand, QuickMatchMode, QuickSource};
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let opts = options(Some(provider.clone()), Arc::new(HeadlessShell::default()));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let view = core.quick_commands_get().await.unwrap();
    assert!(view.commands.is_empty() && view.enabled);
    assert!(view.builtins.len() >= 5);
    let note = CustomCommand {
        id: String::new(),
        name: "Zanotuj".into(),
        phrases: vec!["zanotuj".into()],
        mode: QuickMatchMode::Starts,
        reply: "Zapisuję: {reszta}".into(),
        enabled: true,
        priority: 0,
    };
    let saved = core.quick_commands_set(vec![note.clone()]).await.unwrap();
    assert_eq!(saved.commands[0].id, "zanotuj");
    let t = core
        .quick_commands_test("Zanotuj: kupić mleko".into())
        .await
        .unwrap();
    assert_eq!(t.source, QuickSource::Custom);
    assert_eq!(t.reply.as_deref(), Some("Zapisuję: kupić mleko"));
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let (turn, _) = reply(&core, &sid, "Zanotuj: kupić mleko").await;
    assert_eq!(turn.text, "Zapisuję: kupić mleko");
    assert!(provider.requests().is_empty(), "bez modelu");

    let mut bad = note;
    bad.mode = QuickMatchMode::Regex;
    bad.phrases = vec!["(".into()];
    assert!(core.quick_commands_set(vec![bad]).await.is_err());
    assert_eq!(core.quick_commands_get().await.unwrap().commands.len(), 1);
    core.settings_set("sessions.local_commands".into(), SettingValue::Bool(false))
        .await
        .unwrap();
    let off = core.quick_commands_test("2+2".into()).await.unwrap();
    assert_eq!(off.source, QuickSource::Disabled);
}

/// Przegląd serii 1: offline polecenie nie wyprzedza wiadomości w kolejce, a „ponów” odpowiedzi
/// w quizie nie ocenia odpowiedzi drugi raz (quiz zostaje przy tym samym pytaniu).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queue_order_and_regenerate_keep_quiz_consistent() {
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
    core.set_online(false).await;
    let first = core
        .turns_send(sid.clone(), send("Opowiedz o Krakowie", None))
        .await
        .unwrap();
    assert_eq!(first.assistant_turn_id, None);
    let second = core.turns_send(sid.clone(), send("2+2", None)).await.unwrap();
    assert_eq!(
        second.assistant_turn_id, None,
        "polecenie czeka za wiadomością w kolejce"
    );
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
        "po kolejce odpowiada model — wcześniejsze pytanie nie zostaje bez odpowiedzi"
    );
    core.turns_stop(sid.clone()).await.unwrap();

    let quiz = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let (q1, _) = reply(&core, &quiz, "Przepytaj mnie z tabliczki mnożenia przez 3").await;
    let (a, b) = question(&q1.text);
    let (r1, r1_id) = reply(&core, &quiz, &(a * b).to_string()).await;
    let (c, d) = question(&r1.text);
    let before = provider.requests().len();
    core.turns_regenerate(quiz.clone(), r1_id.unwrap(), None)
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
        "„ponów” liczby nie trafia do quizu"
    );
    core.turns_stop(quiz.clone()).await.unwrap();
    let (next, _) = reply(&core, &quiz, &(c * d).to_string()).await;
    assert!(next.text.starts_with("Dobrze!"), "quiz przy tym samym pytaniu: {}", next.text);
}
