//! Zadania schedulera w rdzeniu (atrapy): DAG dwóch agentek (Beta → Gama) wykonany przez
//! `agent-runtime` z Replay w sesji zadania; kill-switch zatrzymuje trwające zadanie od razu;
//! wyzwalacz ręczny zgłasza zadanie z pochodzeniem `trigger` (dziennik + zdarzenie).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use app_core::dto::{
    AlfaEvent, NewTaskInput, SessionTemplate, SettingValue, TaskInfo, TaskOriginKind,
    TaskResultKind, TaskStateKind, TriggerDraft, TriggerKindView,
};
use app_core::ports::KillOrigin;
use common::{Harness, harness, harness_with, until};

fn input(session: &str, goal: &str, agent: &str, after: Vec<String>) -> NewTaskInput {
    NewTaskInput {
        session_id: Some(session.to_owned()),
        title: String::new(),
        goal: goal.to_owned(),
        agent: Some(agent.to_owned()),
        after,
        parent_id: None,
    }
}

fn update<'a>(e: &'a AlfaEvent, id: &str) -> Option<&'a TaskInfo> {
    match e {
        AlfaEvent::TaskUpdated { task } if task.id == id => Some(task),
        _ => None,
    }
}

fn finished(id: &str) -> impl FnMut(&AlfaEvent) -> bool + '_ {
    move |e| update(e, id).is_some_and(|t| t.state == TaskStateKind::Done)
}

async fn new_session(h: &Harness) -> String {
    h.core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dag_of_two_agents_runs_in_order_with_replay() {
    let mut h = harness().await;
    // Atrapa odpowiada echem — samoweryfikacja „Gotowe” nie przeszłaby (test tego nie dotyczy).
    h.core
        .settings_set(
            "agents.verify_before_done".into(),
            SettingValue::Bool(false),
        )
        .await
        .unwrap();
    let sid = new_session(&h).await;
    let first = h
        .core
        .tasks_create(input(&sid, "Zbierz dane sprzedaży", "beta", vec![]))
        .await
        .unwrap();
    let second = h
        .core
        .tasks_create(input(&sid, "Policz marże", "gama", vec![first.id.clone()]))
        .await
        .unwrap();
    assert_eq!(second.state, TaskStateKind::Pending);
    let events = until(&mut h.rx, finished(&second.id)).await;
    let done_first = events
        .iter()
        .position(|e| update(e, &first.id).is_some_and(|t| t.state == TaskStateKind::Done))
        .expect("pierwsze zadanie zakończone");
    // Most zdarzeń podaje bieżący stan zadania — pierwszy stan poza „czeka” to start B.
    let start_second = events
        .iter()
        .position(|e| update(e, &second.id).is_some_and(|t| t.state != TaskStateKind::Pending))
        .expect("drugie zadanie wystartowało");
    assert!(done_first < start_second, "krawędź DAG: B po A");
    let tasks = h.core.tasks_list().await.unwrap();
    for id in [&first.id, &second.id] {
        let t = tasks.iter().find(|t| &t.id == id).unwrap();
        assert_eq!(t.result, Some(TaskResultKind::Succeeded), "{t:?}");
        assert_eq!(t.origin, TaskOriginKind::User);
    }
    let assignees: Vec<_> = tasks
        .iter()
        .filter(|t| t.id == first.id || t.id == second.id)
        .map(|t| t.assignee.clone())
        .collect();
    assert_eq!(assignees, vec!["beta".to_owned(), "gama".to_owned()]);
    // Replay: przebiegi zadań w sesji zadania (oznaczone identyfikatorem zadania).
    let mut runs = Vec::new();
    for _ in 0..50 {
        runs = h.core.agents_runs(sid.clone()).await.unwrap();
        if runs.len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let ids: Vec<_> = runs.iter().filter_map(|r| r.run.task_id.clone()).collect();
    assert!(
        ids.contains(&first.id) && ids.contains(&second.id),
        "{ids:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn kill_switch_stops_a_running_task_at_once() {
    let mut h = harness_with(Duration::from_millis(40), true).await;
    let sid = new_session(&h).await;
    let goal = "słowo ".repeat(200);
    let task = h
        .core
        .tasks_create(input(&sid, goal.trim(), "delta", vec![]))
        .await
        .unwrap();
    until(&mut h.rx, |e| {
        update(e, &task.id).is_some_and(|t| t.state == TaskStateKind::Running)
    })
    .await;
    let started = std::time::Instant::now();
    h.core.system_kill_all(KillOrigin::Hotkey).await;
    let events = until(&mut h.rx, finished(&task.id)).await;
    let last = events
        .iter()
        .rev()
        .find_map(|e| update(e, &task.id))
        .unwrap();
    assert_eq!(last.result, Some(TaskResultKind::Cancelled), "{last:?}");
    assert!(started.elapsed() < common::budget(500));
    // Ponowienie po kill-switchu to decyzja użytkownika (nowe zadanie, to samo pochodzenie).
    let retried = h.core.tasks_retry(task.id.clone()).await.unwrap();
    assert!(retried.id.contains(".retry"));
    h.core.tasks_cancel(retried.id).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn manual_trigger_submits_a_trigger_task() {
    let mut h = harness().await;
    let created = h
        .core
        .triggers_create(TriggerDraft {
            name: "Porządki".into(),
            kind: TriggerKindView::Manual,
            title: "Porządki".into(),
            goal: "Posprzątaj notatki".into(),
            agent: Some("beta".into()),
            bridge: None,
            respect_dnd: false,
        })
        .await
        .unwrap();
    let run = h.core.triggers_fire_now(created.id.clone()).await.unwrap();
    assert_eq!(run.outcome, "submitted");
    let task_id = run.task_id.unwrap();
    until(&mut h.rx, |e| matches!(e, AlfaEvent::TriggerFired { .. })).await;
    let events = until(&mut h.rx, finished(&task_id)).await;
    let last = events
        .iter()
        .rev()
        .find_map(|e| update(e, &task_id))
        .unwrap();
    assert_eq!(last.origin, TaskOriginKind::Trigger);
    assert_eq!(
        h.core.triggers_log(Some(created.id)).await.unwrap().len(),
        1
    );
    let preview = h
        .core
        .triggers_preview_cron("0 8 * * 1-5".into())
        .await
        .unwrap();
    assert!(preview.valid && preview.next.len() == 5);
}
