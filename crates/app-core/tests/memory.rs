//! Pamięć F7 w rdzeniu (atrapy dostawcy): zapamiętane w projekcie wraca w kontekście innej sesji
//! tego projektu; sesja prywatna nie wycieka do zakresów szerszych ani do innych sesji;
//! usunięcie sesji (crypto-shredding) usuwa wszystko, co z niej pochodzi, także kopie w projekcie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use app_core::dto::{MemoryForgetTarget, MemoryQuery, RememberScope, SessionTemplate};
use common::{Harness, ends, harness, send, until};
use sessions_contract::PrivacyTag;

fn all() -> MemoryQuery {
    MemoryQuery {
        limit: 200,
        ..MemoryQuery::default()
    }
}

async fn session(h: &Harness, project: Option<&str>) -> String {
    let id = h
        .core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    h.core
        .sessions_set_project(id.clone(), project.map(str::to_owned))
        .await
        .unwrap();
    id
}

async fn reply(h: &mut Harness, sid: &str, text: &str) -> String {
    let sent = h
        .core
        .turns_send(sid.into(), send(text, None))
        .await
        .unwrap();
    let turn = sent.assistant_turn_id.unwrap();
    until(&mut h.rx, ends(&turn)).await;
    turn
}

fn system_of_last(h: &Harness, sid: &str) -> String {
    h.provider
        .requests()
        .into_iter()
        .rev()
        .find(|r| r.meta.session.as_deref() == Some(sid))
        .and_then(|r| r.system)
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn project_memory_returns_in_another_session_of_the_project() {
    let mut h = harness().await;
    let a = session(&h, Some("Projekt X")).await;
    let b = session(&h, Some("Projekt X")).await;
    let other = session(&h, Some("Projekt Y")).await;
    let turn = reply(&mut h, &a, "Faktury wysyłamy zawsze w czwartki").await;
    h.core
        .turns_remember(turn, RememberScope::Project)
        .await
        .unwrap();
    reply(&mut h, &b, "Kiedy wysyłamy faktury?").await;
    assert!(
        system_of_last(&h, &b).contains("Faktury wysyłamy zawsze w czwartki"),
        "pamięć projektu w kontekście sesji B: {}",
        system_of_last(&h, &b)
    );
    reply(&mut h, &other, "Kiedy wysyłamy faktury?").await;
    assert!(!system_of_last(&h, &other).contains("czwartki"));
    let scopes = h.core.memory_scopes().await.unwrap();
    assert!(scopes.iter().any(|s| s.key.starts_with("project:")));
    let page = h.core.memory_inspect(all()).await.unwrap();
    let item = page
        .items
        .iter()
        .find(|i| i.text.contains("czwartki"))
        .unwrap();
    let why = h.core.memory_explain(item.id.clone()).await.unwrap();
    assert!(!why.reasons.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn private_session_memory_never_leaves_the_session() {
    let mut h = harness().await;
    let private = session(&h, Some("Projekt X")).await;
    h.core
        .sessions_set_privacy(private.clone(), PrivacyTag::Private)
        .await
        .unwrap();
    let other = session(&h, Some("Projekt X")).await;
    let turn = reply(&mut h, &private, "Kod do sejfu to 4711").await;
    h.core
        .turns_remember(turn.clone(), RememberScope::Session)
        .await
        .unwrap();
    for scope in [
        RememberScope::Project,
        RememberScope::Global,
        RememberScope::Agent,
    ] {
        assert!(
            h.core.turns_remember(turn.clone(), scope).await.is_err(),
            "zapis z sesji prywatnej do zakresu szerszego: {scope:?}"
        );
    }
    reply(&mut h, &other, "Jaki jest kod do sejfu?").await;
    for request in h.provider.requests() {
        if request.meta.session.as_deref() == Some(private.as_str()) {
            continue;
        }
        assert!(
            !format!("{request:?}").contains("4711"),
            "wyciek pamięci prywatnej do innej sesji"
        );
    }
    let exportable = h.core.memory_scopes().await.unwrap();
    let own = exportable
        .iter()
        .find(|s| s.key == format!("session:{private}"))
        .unwrap();
    assert!(own.document.is_none(), "sesja prywatna poza eksportem");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn removing_a_session_forgets_its_memory_and_copies() {
    let mut h = harness().await;
    let sid = session(&h, Some("Projekt Z")).await;
    let turn = reply(&mut h, &sid, "Klient woli kontakt mailowy").await;
    for scope in [RememberScope::Session, RememberScope::Project] {
        h.core.turns_remember(turn.clone(), scope).await.unwrap();
    }
    let before = h.core.memory_inspect(all()).await.unwrap();
    assert_eq!(
        before
            .items
            .iter()
            .filter(|i| i.text.contains("mailowy"))
            .count(),
        2
    );
    let preview = h
        .core
        .memory_forget_preview(MemoryForgetTarget::Scope {
            scope: format!("session:{sid}"),
        })
        .await
        .unwrap();
    // Zakres sesji żyje w bazie sesji — znika z nią (crypto-shredding w `sessions`).
    assert!(preview.remove.iter().any(|r| r.text.contains("mailowy")));
    h.core.sessions_remove(sid.clone()).await.unwrap();
    let mut left = usize::MAX;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let page = h.core.memory_inspect(all()).await.unwrap();
        left = page
            .items
            .iter()
            .filter(|i| i.text.contains("mailowy"))
            .count();
        if left == 0 {
            break;
        }
    }
    assert_eq!(left, 0, "kopie z usuniętej sesji zostały w pamięci");
    let scopes = h.core.memory_scopes().await.unwrap();
    assert!(!scopes.iter().any(|s| s.key == format!("session:{sid}")));
}
