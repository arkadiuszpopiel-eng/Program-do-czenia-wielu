//! Eksport rozmowy do Markdown / HTML: aktywna gałąź bez tur ukrytych i systemowych, jedna
//! wiadomość, załączniki, HTML bez skryptów (XSS z treści i tytułu), natywny dialog zapisu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_api::dto::{ConversationFormat, ExportResult};
use app_api::ids;
use app_files::export;
use common::Env;
use sessions_contract::{NewTurn, Role, SessionCatalog, SessionHistory, SessionPatch};

fn conversation(env: &Env) -> sessions_contract::SessionId {
    let s = env.session("Rozmowa");
    let h = &env.sessions;
    let a = h
        .append_turn(&s, None, NewTurn::user("Pytanie o **raport**"))
        .unwrap();
    let b = h
        .append_turn(
            &s,
            Some(a.id),
            NewTurn::assistant("alfa", "Odpowiedź:\n\n```sql\nSELECT 1;\n```"),
        )
        .unwrap();
    let mut hidden = NewTurn::user("ukryta wiadomość");
    hidden.content.text = "ukryta wiadomość".into();
    let c = h.append_turn(&s, Some(b.id), hidden).unwrap();
    h.set_hidden(&s, c.id, true).unwrap();
    let mut system = NewTurn::assistant("alfa", "komunikat systemowy");
    system.role = Role::System;
    let d = h.append_turn(&s, Some(c.id), system).unwrap();
    h.append_turn(
        &s,
        Some(d.id),
        NewTurn::assistant(
            "delta",
            "<img src=x onerror=alert(1)><script>alert(2)</script>Gotowe",
        ),
    )
    .unwrap();
    env.sessions
        .update_session(
            &s,
            SessionPatch {
                title: Some("Raport <script>alert(3)</script> Q3".into()),
                ..SessionPatch::default()
            },
        )
        .unwrap();
    s
}

#[tokio::test]
async fn markdown_has_visible_branch_only() {
    let env = Env::new();
    let s = conversation(&env);
    let path = env.dir.path().join("rozmowa.md");
    env.shell.answer_dialog(Some(path.clone()));
    let result = env
        .app
        .export_conversation(s.as_str(), ConversationFormat::Markdown, None)
        .await
        .unwrap();
    let ExportResult::Saved { files, bytes, .. } = result else {
        panic!("zapisano");
    };
    assert_eq!(files, 1);
    let md = std::fs::read_to_string(&path).unwrap();
    assert_eq!(bytes, md.len() as u64);
    assert!(md.starts_with("# Raport"));
    assert!(md.contains("### Ty ·") && md.contains("### Alfa ·") && md.contains("### Delta ·"));
    assert!(md.contains("```sql\nSELECT 1;\n```"));
    assert!(!md.contains("ukryta wiadomość"), "tury ukryte pomijane");
    assert!(!md.contains("komunikat systemowy"));
    assert!(md.contains("wiadomości: 3"));
    assert!(
        env.shell
            .calls()
            .iter()
            .any(|c| c.starts_with("pick_save_file:Raport scriptalert3script Q3"))
    );
}

#[tokio::test]
async fn html_is_sanitized_standalone_document() {
    let env = Env::new();
    let s = conversation(&env);
    let path = env.dir.path().join("rozmowa.html");
    env.shell.answer_dialog(Some(path.clone()));
    env.app
        .export_conversation(s.as_str(), ConversationFormat::Html, None)
        .await
        .unwrap();
    let html = std::fs::read_to_string(&path).unwrap();
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("Content-Security-Policy\" content=\"default-src 'none'"));
    assert!(!html.contains("<script"), "{html}");
    assert!(!html.contains("<img"), "surowy HTML z modelu escapowany");
    assert!(
        html.contains("&lt;img src=x onerror=alert(1)&gt;"),
        "{html}"
    );
    assert!(html.contains("&lt;script&gt;alert(3)"));
    assert!(html.contains("<pre"));
    assert!(
        !html.contains("http://") && !html.contains("https://"),
        "bez zasobów zewnętrznych"
    );
    assert!(!html.contains("@font-face"));
}

#[tokio::test]
async fn single_message_cancel_and_errors() {
    let env = Env::new();
    let s = conversation(&env);
    let first = ids::turn_dto(&s, sessions_contract::TurnId(1));
    let path = env.dir.path().join("jedna.md");
    env.shell.answer_dialog(Some(path.clone()));
    env.app
        .export_conversation(s.as_str(), ConversationFormat::Markdown, Some(first))
        .await
        .unwrap();
    let md = std::fs::read_to_string(&path).unwrap();
    assert!(md.contains("wiadomości: 1") && md.contains("raport"));
    assert!(!md.contains("Gotowe"));
    env.shell.answer_dialog(None);
    let cancelled = env
        .app
        .export_conversation(s.as_str(), ConversationFormat::Html, None)
        .await
        .unwrap();
    assert_eq!(cancelled, ExportResult::Cancelled);
    let other = env.session("Pusta");
    assert!(
        env.app
            .export_conversation(other.as_str(), ConversationFormat::Markdown, None)
            .await
            .is_err()
    );
    let foreign = ids::turn_dto(&other, sessions_contract::TurnId(1));
    assert!(
        env.app
            .export_conversation(s.as_str(), ConversationFormat::Markdown, Some(foreign))
            .await
            .is_err()
    );
}

#[test]
fn file_names_are_safe() {
    let now = chrono::Utc::now();
    let name = export::file_name("..\\..\\a/b:c", now, "md");
    assert!(!name.contains(['\\', '/', ':']));
    assert!(export::file_name("", now, "html").starts_with("rozmowa "));
    assert_eq!(
        export::escape("<a href=\"x\">'&"),
        "&lt;a href=&quot;x&quot;&gt;&#39;&amp;"
    );
}
