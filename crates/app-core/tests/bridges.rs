//! Mosty CLI w rdzeniu na `agent-backends-fake` (prawdziwe reguły pochodzenia): delegacja z czatu
//! („Delta, zleć to Claude Code") → zadanie mostu od użytkownika, Replay „niezweryfikowane przez
//! Alfę", wynik z dopiskiem; wyzwalacz nigdy nie uruchamia mostu (0 startów); karty zgodności,
//! zgoda na harmonogram (limit 24/dobę) i „Zaloguj w terminalu" (polecenie do skopiowania).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_backends_contract::{AgentBackend, LaunchOrigin};
use agent_backends_fake::FakeAgentBackend;
use app_core::dto::{
    AlfaEvent, SessionTemplate, TaskResultKind, TriggerDraft, TriggerKindView, WorkdirChoice,
};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::agents::to_delta;
use common::{Harness, ScriptedProvider, ends, options, until};

type Slot = Arc<Mutex<Option<Arc<FakeAgentBackend>>>>;

async fn bridged() -> (Harness, Slot, String) {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(HeadlessShell::default());
    let slot: Slot = Arc::default();
    let mut opts = options(Some(provider.clone()), shell.clone());
    let fill = slot.clone();
    opts.bridges = Some(Arc::new(move |sink| {
        let backend = Arc::new(FakeAgentBackend::new(sink));
        *fill.lock().unwrap() = Some(backend.clone());
        backend as Arc<dyn AgentBackend>
    }));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let rx = core.subscribe_events();
    let sid = core
        .sessions_create(SessionTemplate::Coding)
        .await
        .unwrap()
        .id;
    core.sessions_choose_workdir(sid.clone(), WorkdirChoice::Default)
        .await
        .unwrap();
    let h = Harness {
        core,
        provider,
        shell,
        rx,
        dir,
    };
    (h, slot, sid)
}

fn submitted(slot: &Slot) -> usize {
    slot.lock()
        .unwrap()
        .as_ref()
        .map_or(0, |b| b.submitted().len())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn chat_delegation_runs_a_user_bridge_task_marked_unverified() {
    let (mut h, slot, sid) = bridged().await;
    let sent = h
        .core
        .turns_send(
            sid.clone(),
            to_delta("Delta, zleć to Claude Code: popraw testy płatności"),
        )
        .await
        .unwrap();
    let turn = sent.assistant_turn_id.unwrap();
    let events = until(&mut h.rx, ends(&turn)).await;
    let text: String = events
        .iter()
        .filter_map(|e| match e {
            AlfaEvent::TextDelta { turn_id, text, .. } if turn_id == &turn => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(text.contains("Gotowe."), "wynik mostu w turze: {text}");
    assert!(text.contains("niezweryfikowany przez Alfę"), "{text}");
    let backend = slot.lock().unwrap().clone().unwrap();
    let specs = backend.submitted();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].origin, LaunchOrigin::UserRequest);
    assert!(specs[0].prompt.contains("popraw testy płatności"));
    // Delegacja to nie rozmowa z modelem: dostawca nie dostał tej wiadomości.
    assert!(h.provider.requests().is_empty());
    let runs = h.core.agents_runs(sid.clone()).await.unwrap();
    let run = runs
        .iter()
        .find(|r| r.run.bridge.as_deref() == Some("claude_code"))
        .expect("przebieg mostu w Replay");
    assert!(run.steps.iter().all(|s| s.untrusted));
    let tasks = h.core.tasks_list().await.unwrap();
    let task = tasks
        .iter()
        .find(|t| t.executor == "bridge:claude_code")
        .unwrap();
    assert_eq!(task.result, Some(TaskResultKind::Succeeded));
    assert_eq!(run.run.task_id.as_deref(), Some(task.id.as_str()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn triggers_never_start_a_bridge() {
    let (mut h, slot, _sid) = bridged().await;
    let draft = |kind| TriggerDraft {
        name: "Most z wyzwalacza".into(),
        kind,
        title: "Most".into(),
        goal: "Popraw testy".into(),
        agent: Some("delta".into()),
        bridge: Some("claude_code".into()),
        respect_dnd: false,
    };
    let event = draft(TriggerKindView::FileInDir {
        dir: "C:\\Pobrane".into(),
        pattern: None,
    });
    assert!(h.core.triggers_create(event).await.is_err());
    let manual = draft(TriggerKindView::Manual);
    assert!(h.core.triggers_create(manual).await.is_err());
    // Regresja CX-d (AGENTS.md): harmonogram czasowy z mostem — odrzucony także przy niezerowej
    // zgodzie dziennej na trasie; most nie startuje.
    h.core
        .bridges_set_schedule("claude_code".into(), 24)
        .await
        .unwrap();
    let cron = draft(TriggerKindView::Cron {
        expr: "0 3 * * *".into(),
    });
    assert!(h.core.triggers_create(cron).await.is_err());
    let plain = TriggerDraft {
        bridge: None,
        ..draft(TriggerKindView::Cron {
            expr: "0 3 * * *".into(),
        })
    };
    let created = h.core.triggers_create(plain).await.unwrap();
    assert_eq!(created.bridge, None);
    let run = h.core.triggers_fire_now(created.id).await.unwrap();
    if let Some(task) = run.task_id {
        until(&mut h.rx, |e| {
            matches!(e, AlfaEvent::TaskUpdated { task: t } if t.id == task && t.result.is_some())
        })
        .await;
    }
    assert_eq!(submitted(&slot), 0, "most wystartował z wyzwalacza");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compliance_cards_schedule_consent_and_login_command() {
    let (h, _slot, _sid) = bridged().await;
    let cards = h.core.bridges_list(false).await.unwrap();
    let claude = cards
        .iter()
        .find(|c| c.route_id == "claude-code-cli")
        .expect("karta Claude Code");
    assert_eq!(claude.bridge.as_deref(), Some("claude_code"));
    assert!(claude.verified_at.is_some());
    assert!(cards.iter().all(|c| c.mode != "api"));
    let consent = h
        .core
        .bridges_set_schedule("claude_code".into(), 99)
        .await
        .unwrap();
    assert_eq!(consent.schedule_per_day, 24);
    assert!(
        h.core
            .bridges_pin("claude_code".into(), Some("9.9.9".into()))
            .await
            .is_err(),
        "przypiąć można tylko wykrytą wersję"
    );
    let login = h
        .core
        .bridges_open_login("claude_code".into())
        .await
        .unwrap();
    assert_eq!(login.command, "claude /login");
    assert!(login.opened);
    assert!(
        h.shell
            .calls()
            .iter()
            .any(|c| c.starts_with("open_terminal:"))
    );
    assert!(h.core.bridges_open_login("nieznany".into()).await.is_err());
}
