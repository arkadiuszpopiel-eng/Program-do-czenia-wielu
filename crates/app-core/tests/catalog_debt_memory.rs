//! Dług katalogu funkcji (docs/plan/45-testy-ai.md): „Naucz Alfę” na rdzeniu — tekst i plik to
//! treść niezaufana tylko w pamięci rozmowy, rozmowa (tury użytkownika) może trafić wyżej,
//! złe żądania są odrzucane bez zapisu. Wybór pliku idzie przez powłokę bez okien
//! (`HeadlessShell`): odpowiedź dialogu podaje test, bez niej — „niedostępne”.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;

use app_core::ErrorCode;
use app_core::dto::{MemoryQuery, RememberScope, SessionTemplate, TeachRequest, TeachSource};
use common::{Harness, ends, harness, send, until};

fn all() -> MemoryQuery {
    MemoryQuery {
        limit: 200,
        ..MemoryQuery::default()
    }
}

async fn new_session(h: &Harness) -> String {
    h.core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id
}

fn teach(session: &str, source: TeachSource, scope: RememberScope) -> TeachRequest {
    TeachRequest {
        session_id: session.to_owned(),
        source,
        scope,
        title: None,
    }
}

fn text(t: &str) -> TeachSource {
    TeachSource::Text { text: t.into() }
}

async fn stored(h: &Harness) -> Vec<app_core::dto::MemoryItem> {
    h.core.memory_inspect(all()).await.unwrap().items
}

fn file(h: &Harness, name: &str, content: &[u8]) -> PathBuf {
    let dir = h.dir.path().join("Dokumenty");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, content).unwrap();
    path
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_teach_stores_pasted_text_as_untrusted_session_facts() {
    let h = harness().await;
    let sid = new_session(&h).await;
    let mut request = teach(
        &sid,
        text("Faktury wysyłamy w czwartki.\n\nMagazyn zamykamy o 17:00."),
        RememberScope::Session,
    );
    request.title = Some("Zasady [biuro]".into());
    let report = h.core.memory_teach(request).await.unwrap();
    assert!(report.facts >= 1, "{report:?}");
    assert_eq!(report.ids.len() as u64, report.facts);
    assert_eq!(report.scope_key, format!("session:{sid}"));
    assert!(!report.trusted, "wklejony tekst jest niezaufany");
    assert!(report.source.contains("wklejony tekst"), "{report:?}");

    let items = stored(&h).await;
    let facts: Vec<_> = items
        .iter()
        .filter(|i| report.ids.contains(&i.id))
        .collect();
    assert_eq!(facts.len() as u64, report.facts);
    assert!(
        facts
            .iter()
            .all(|i| !i.trusted && i.scope_key == report.scope_key)
    );
    let joined: String = facts.iter().map(|i| i.text.as_str()).collect();
    assert!(
        joined.contains("[Zasady biuro"),
        "tytuł bez nawiasów: {joined}"
    );
    assert!(
        joined.contains("czwartki") && joined.contains("17:00"),
        "{joined}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_teach_refuses_bad_requests_without_writing() {
    let h = harness().await;
    let sid = new_session(&h).await;
    let refused = [
        teach(&sid, text("Treść z zewnątrz"), RememberScope::Global),
        teach(&sid, text("Treść z zewnątrz"), RememberScope::Project),
        teach("   ", text("Cokolwiek"), RememberScope::Session),
        teach(&sid, text("   \n\n  "), RememberScope::Session),
        teach(&sid, TeachSource::Conversation, RememberScope::Session),
        teach(&sid, TeachSource::Conversation, RememberScope::Agent),
        teach(
            &sid,
            text(&"a".repeat(4 * 1024 * 1024 + 1)),
            RememberScope::Session,
        ),
    ];
    for request in refused {
        let what = format!("{:?}/{:?}", request.scope, request.session_id);
        let err = h.core.memory_teach(request).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{what}: {err:?}");
    }
    let unknown = h
        .core
        .memory_teach(teach("nie-ma-takiej", text("x"), RememberScope::Session))
        .await
        .unwrap_err();
    assert_eq!(unknown.code, ErrorCode::NotFound, "{unknown:?}");
    assert!(stored(&h).await.is_empty(), "odmowa niczego nie zapisuje");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_teach_from_conversation_keeps_only_user_turns_globally() {
    let mut h = harness().await;
    let sid = new_session(&h).await;
    let sent = h
        .core
        .turns_send(sid.clone(), send("Mój ulubiony kolor to zielony", None))
        .await
        .unwrap();
    until(&mut h.rx, ends(&sent.assistant_turn_id.unwrap())).await;

    let report = h
        .core
        .memory_teach(teach(
            &sid,
            TeachSource::Conversation,
            RememberScope::Global,
        ))
        .await
        .unwrap();
    assert_eq!(report.scope_key, "global");
    assert!(report.trusted, "same tury użytkownika: {report:?}");
    assert_eq!(report.source, format!("rozmowa:{sid}"));
    let items = stored(&h).await;
    let facts: Vec<_> = items
        .iter()
        .filter(|i| report.ids.contains(&i.id))
        .collect();
    let joined: String = facts.iter().map(|i| i.text.as_str()).collect();
    assert!(
        joined.contains("Użytkownik: Mój ulubiony kolor"),
        "{joined}"
    );
    assert!(
        !joined.contains("Echo:"),
        "odpowiedź agentki nie idzie wyżej: {joined}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_teach_pick_file_reads_text_through_the_shell_dialog() {
    let h = harness().await;
    // Bez odpowiedzi dialogu (brak okien) — „niedostępne”, nie błąd wewnętrzny.
    let err = h.core.memory_teach_pick_file().await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Unavailable, "{err:?}");
    // Anulowany wybór.
    h.shell.answer_dialog(None);
    assert_eq!(h.core.memory_teach_pick_file().await.unwrap(), None);

    let notes = file(&h, "Notatki.md", "# Plan\n\nSpotkanie w piątek.".as_bytes());
    h.shell.answer_dialog(Some(notes));
    let picked = h.core.memory_teach_pick_file().await.unwrap().unwrap();
    assert_eq!(picked.name, "Notatki.md");
    assert_eq!(picked.text, "# Plan\n\nSpotkanie w piątek.");
    assert_eq!(picked.chars, picked.text.chars().count() as u64);
    assert!(!picked.truncated);
    let calls = h.shell.calls();
    assert_eq!(
        calls.iter().filter(|c| c.as_str() == "pick_files").count(),
        3,
        "{calls:?}"
    );

    // Wybrany plik trafia do nauki jako plik (treść niezaufana, w pamięci rozmowy).
    let sid = new_session(&h).await;
    let source = TeachSource::File {
        name: picked.name,
        text: picked.text,
    };
    let report = h
        .core
        .memory_teach(teach(&sid, source, RememberScope::Session))
        .await
        .unwrap();
    assert!(
        report.source.contains("Notatki.md") && !report.trusted,
        "{report:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_teach_pick_file_refuses_unsupported_empty_and_protected_files() {
    let h = harness().await;
    let protected = h.dir.path().join("local").join("state");
    std::fs::create_dir_all(&protected).unwrap();
    let inside = protected.join("tajne.md");
    std::fs::write(&inside, "Dane Alfy").unwrap();
    for path in [
        file(&h, "program.exe", b"MZ\x90\x00binarka"),
        file(&h, "pusty.txt", b""),
        file(&h, "binarny.txt", &[0u8, 159, 146, 150, 0, 0, 1, 2]),
        inside,
        h.dir.path().join("Dokumenty").join("brak.md"),
    ] {
        h.shell.answer_dialog(Some(path.clone()));
        let err = h.core.memory_teach_pick_file().await.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{path:?}: {err:?}");
    }
}
