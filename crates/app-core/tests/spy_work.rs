//! Test szpiegowski F5/F7: 3 sesje w różnych projektach, każda ze swoim sekretem w pamięci
//! (zakres sesji i projektu) i w zadaniu schedulera — 0 przecieków: w kontekście modelu
//! (pamięć agentki), w Replay przebiegów zadań, w zdarzeniach `TaskUpdated` / `AgentStep`
//! i w Inspektorze pamięci filtrowanym po zakresie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use app_core::dto::{
    AlfaEvent, MemoryQuery, NewTaskInput, RememberScope, SessionTemplate, SettingValue,
    TaskStateKind,
};
use common::{ends, harness_with, send, until};

const SESSIONS: usize = 3;

fn leaks(text: &str, tokens: &[String], own: usize) -> Vec<usize> {
    tokens
        .iter()
        .enumerate()
        .filter(|(i, t)| *i != own && text.contains(t.as_str()))
        .map(|(i, _)| i)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn memory_and_tasks_never_leak_between_sessions() {
    let mut h = harness_with(Duration::from_millis(1), true).await;
    h.core
        .settings_set(
            "agents.verify_before_done".into(),
            SettingValue::Bool(false),
        )
        .await
        .unwrap();
    let tokens: Vec<String> = (0..SESSIONS)
        .map(|i| format!("TAJNE{i}Q{}", uuid::Uuid::new_v4().simple()))
        .collect();
    let mut ids = Vec::new();
    for i in 0..SESSIONS {
        let sid = h
            .core
            .sessions_create(SessionTemplate::Empty)
            .await
            .unwrap()
            .id;
        h.core
            .sessions_set_project(sid.clone(), Some(format!("Projekt {i}")))
            .await
            .unwrap();
        ids.push(sid);
    }
    let mut events: Vec<AlfaEvent> = Vec::new();
    let mut tasks = Vec::new();
    for (i, sid) in ids.iter().enumerate() {
        let sent = h
            .core
            .turns_send(
                sid.clone(),
                send(&format!("Notatka: kod {} jest ważny", tokens[i]), None),
            )
            .await
            .unwrap();
        let turn = sent.assistant_turn_id.unwrap();
        events.extend(until(&mut h.rx, ends(&turn)).await);
        for scope in [RememberScope::Session, RememberScope::Project] {
            h.core.turns_remember(turn.clone(), scope).await.unwrap();
        }
        let task = h
            .core
            .tasks_create(NewTaskInput {
                session_id: Some(sid.clone()),
                title: format!("Zadanie {i}"),
                goal: format!("Sprawdź kod {}", tokens[i]),
                agent: Some("beta".into()),
                after: Vec::new(),
                parent_id: None,
            })
            .await
            .unwrap();
        tasks.push(task.id);
    }
    // Zadania mogły skończyć się w trakcie wcześniejszych oczekiwań — stan z listy zadań.
    for _ in 0..400 {
        let list = h.core.tasks_list().await.unwrap();
        let done = tasks.iter().all(|id| {
            list.iter()
                .any(|t| &t.id == id && t.state == TaskStateKind::Done)
        });
        if done {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    while let Ok(batch) = h.rx.try_recv() {
        events.extend(batch.iter().cloned());
    }
    // Pytanie bez sekretu w każdej sesji — kontekst pamięci tylko z własnych zakresów.
    for sid in &ids {
        let sent = h
            .core
            .turns_send(sid.clone(), send("Co pamiętasz o kodzie?", None))
            .await
            .unwrap();
        events.extend(until(&mut h.rx, ends(&sent.assistant_turn_id.unwrap())).await);
    }
    for request in h.provider.requests() {
        let Some(own) = request
            .meta
            .session
            .as_deref()
            .and_then(|s| ids.iter().position(|x| x == s))
        else {
            continue;
        };
        let text = format!("{request:?}");
        assert!(leaks(&text, &tokens, own).is_empty(), "żądanie sesji {own}");
    }
    for event in &events {
        let (session, text) = match event {
            AlfaEvent::TaskUpdated { task } => (task.session_id.clone(), format!("{task:?}")),
            AlfaEvent::AgentStep {
                session_id, step, ..
            } => (Some(session_id.clone()), format!("{step:?}")),
            AlfaEvent::AgentRunUpdated { session_id, run } => {
                (Some(session_id.clone()), format!("{run:?}"))
            }
            _ => continue,
        };
        let Some(own) = session.and_then(|s| ids.iter().position(|x| *x == s)) else {
            continue;
        };
        assert!(
            leaks(&text, &tokens, own).is_empty(),
            "zdarzenie sesji {own}"
        );
    }
    let scopes = h.core.memory_scopes().await.unwrap();
    for (i, sid) in ids.iter().enumerate() {
        let runs = h.core.agents_runs(sid.clone()).await.unwrap();
        assert!(
            leaks(&format!("{runs:?}"), &tokens, i).is_empty(),
            "Replay sesji {i}"
        );
        let project = scopes
            .iter()
            .find(|s| {
                s.label.contains(&format!("Projekt {i}"))
                    || s.key.ends_with(&format!("projekt-{i}"))
            })
            .map(|s| s.key.clone());
        let mut keys = vec![format!("session:{sid}")];
        keys.extend(project);
        for key in keys {
            let page = h
                .core
                .memory_inspect(MemoryQuery {
                    scopes: vec![key.clone()],
                    limit: 100,
                    ..MemoryQuery::default()
                })
                .await
                .unwrap();
            assert!(!page.items.is_empty(), "zakres {key} pusty");
            let text = format!("{:?}", page.items);
            assert!(leaks(&text, &tokens, i).is_empty(), "pamięć {key}");
        }
    }
}
