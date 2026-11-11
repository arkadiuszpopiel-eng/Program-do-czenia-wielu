//! Ścieżka Core → dostawca: zapis faktów jako dane, bez dopisywania wywołań modelu.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
#[path = "support/history_seed.rs"]
mod history_seed;

use app_core::dto::{SessionTemplate, SettingValue, WorkdirChoice};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, ends, send, until};
use providers_contract::{ChatRequest, ContentBlock, Role};
use providers_fake::{FAKE_MODEL, Script};
use std::{sync::Arc, time::Duration};

const MARK: &str = "[Zapis czynności aplikacji]";

fn records(request: &ChatRequest) -> Option<serde_json::Value> {
    let mut found = request.messages.iter().filter_map(|m| {
        let text = m.visible_text();
        let data = text.strip_prefix(MARK)?.trim();
        assert_eq!(m.role, Role::User, "dane nie są nowym System ani Assistant");
        assert!(
            m.content
                .iter()
                .all(|b| matches!(b, ContentBlock::Text { .. }))
        );
        assert!(data.len() <= 1536);
        Some(serde_json::from_str::<serde_json::Value>(data).unwrap())
    });
    let value = found.next();
    assert!(
        found.next().is_none(),
        "jeden odczyt bez kumulowania poprzednich not"
    );
    if let Some(v) = &value {
        assert_eq!(v["source"], "application_execution_records");
        assert_eq!(v["scope"], "selected_branch_before_current_turn");
        assert_eq!(v["lookup_is_not_agent_action"], true);
    }
    value
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn branch_records_reach_provider_without_rewriting_history_or_creating_actions() {
    let dir = tempfile::tempdir().unwrap();
    let paths = AppPaths::under(dir.path());
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let opts = common::options(Some(provider.clone()), Arc::new(HeadlessShell::default()));
    let secrets = opts.secrets.clone().unwrap();
    let core = AppCore::build(paths.clone(), opts).await.unwrap();
    let mut events = core.subscribe_events();
    let fresh = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    core.sessions_choose_workdir(fresh.clone(), WorkdirChoice::None)
        .await
        .unwrap();
    provider.push(Script::text(FAKE_MODEL, &["Pierwsza odpowiedź."]));
    let turn = core
        .turns_send(fresh, send(history_seed::QUERY, None))
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut events, ends(&turn)).await;
    assert!(records(provider.requests().last().unwrap()).is_none());
    for scenario in ["empty_record", "recorded_done", "needs_confirmation"] {
        let sid = history_seed::seed(&core, &paths, secrets.clone(), "delta", scenario).await;
        let prior = core.turns_list(sid.clone()).await.unwrap();
        provider.push(Script::text(FAKE_MODEL, &["Odpowiedź bez podmiany."]));
        let mut message = send(history_seed::QUERY, None);
        message.addressed_to = Some("delta".into());
        let turn = core
            .turns_send(sid.clone(), message)
            .await
            .unwrap()
            .assistant_turn_id
            .unwrap();
        until(&mut events, ends(&turn)).await;
        let request = provider.requests().last().cloned().unwrap();
        assert_eq!(
            request.messages.last().unwrap().visible_text(),
            history_seed::QUERY
        );
        let json = serde_json::to_string(&request).unwrap();
        assert!(!json.contains("sekret-testowy"));
        assert!(
            request
                .messages
                .iter()
                .flat_map(|m| &m.content)
                .all(|b| matches!(b, ContentBlock::Text { .. }))
        );
        let facts = records(&request).expect("brak rzeczywistych zapisów gałęzi w żądaniu");
        capture(scenario, &request);
        assert_eq!(
            facts["counts"]["recorded_done"],
            u64::from(scenario == "recorded_done")
        );
        assert_eq!(
            facts["counts"]["needs_confirmation"],
            u64::from(scenario == "needs_confirmation")
        );
        assert_eq!(facts["counts"]["unverified"], 0);
        let turns = core.turns_list(sid.clone()).await.unwrap();
        for old in &prior.turns {
            let after = turns.turns.iter().find(|t| t.id == old.id).unwrap();
            assert_eq!(
                serde_json::to_value(after).unwrap(),
                serde_json::to_value(old).unwrap()
            );
            assert!(
                request
                    .messages
                    .iter()
                    .any(|m| m.visible_text().contains(&old.text))
            );
        }
        let answer = turns.turns.iter().find(|t| t.id == turn).unwrap();
        assert!(answer.tools.is_empty(), "odczyt nie jest akcją agentki");
        assert_eq!(answer.text, "Odpowiedź bez podmiany.");
        // Fork przed dawną odpowiedzią nie dziedziczy jej narzędzi ani tekstu.
        provider.push(Script::text(FAKE_MODEL, &["Odpowiedź innej gałęzi."]));
        let parent = prior.turns.first().unwrap().id.clone();
        let turn = core
            .turns_send(sid, send("Pytanie w nowej gałęzi", Some(parent)))
            .await
            .unwrap()
            .assistant_turn_id
            .unwrap();
        until(&mut events, ends(&turn)).await;
        assert!(records(provider.requests().last().unwrap()).is_none());
    }
}

// Opcjonalny eksport żądań wyłącznie z syntetycznego fixture do lokalnego pilota modelu.
fn capture(scenario: &str, request: &ChatRequest) {
    if let Some(root) = std::env::var_os("ALFA_RECORDS_CAPTURE") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(format!("{scenario}.json")),
            serde_json::to_vec_pretty(request).unwrap(),
        )
        .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_write_and_undo_are_projected_independently_of_the_answer() {
    use common::agents::run_turn;
    let (mut a, secrets) = restartable_agents().await;
    std::fs::write(a.workdir.join("fact.txt"), "wcześniejsza treść").unwrap();
    a.h.core
        .settings_set(
            "agents.interaction_mode".into(),
            SettingValue::Text("jarvis".into()),
        )
        .await
        .unwrap();
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "read-1",
        "fs_read",
        &serde_json::json!({"path":"fact.txt"}),
    ));
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "write-1",
        "fs_write",
        &serde_json::json!({"path":"fact.txt", "mode":"overwrite", "content":"testowy wynik"}),
    ));
    a.h.provider.push(Script::text(
        FAKE_MODEL,
        &["Nie mam potwierdzenia wykonania."],
    ));
    let (turn, _) = run_turn(&mut a, "Zapisz fact.txt z podaną treścią.").await;
    assert_eq!(
        std::fs::read_to_string(a.workdir.join("fact.txt")).unwrap(),
        "testowy wynik"
    );
    let turns = a.h.core.turns_list(a.sid.clone()).await.unwrap();
    let token = turns
        .turns
        .iter()
        .find(|t| t.id == turn)
        .unwrap()
        .tools
        .iter()
        .find_map(|s| s.undo_token.clone())
        .unwrap();
    for undone in [false, true] {
        if undone {
            // Nowy Core nad tą samą zaszyfrowaną historią i sejfem fixture.
            drop(a.h.core);
            let mut opts = common::options(Some(a.h.provider.clone()), a.h.shell.clone());
            opts.secrets = Some(secrets.clone());
            opts.exec = Some(a.exec.clone());
            a.h.core = AppCore::build(AppPaths::under(a.h.dir.path()), opts)
                .await
                .unwrap();
            a.h.rx = a.h.core.subscribe_events();
            assert_eq!(
                std::fs::read_to_string(a.workdir.join("fact.txt")).unwrap(),
                "testowy wynik"
            );
            a.h.core.turns_undo_step(token.clone()).await.unwrap();
            assert_eq!(
                std::fs::read_to_string(a.workdir.join("fact.txt")).unwrap(),
                "wcześniejsza treść"
            );
        }
        a.h.provider.push(Script::text(
            FAKE_MODEL,
            &["Meldunek bez dodatkowych działań."],
        ));
        let (answer, _) = run_turn(&mut a, history_seed::QUERY).await;
        let request = a.h.provider.requests().last().cloned().unwrap();
        assert_eq!(
            request.messages.last().unwrap().visible_text(),
            format!("Zadanie od właściciela: {}", history_seed::QUERY)
        );
        let facts = records(&request).expect("brak zapisu wykonanego fs_write");
        assert_eq!(facts["counts"]["recorded_done"], 1 + u64::from(!undone));
        assert_eq!(facts["counts"]["undone"], u64::from(undone));
        let record = facts["records"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["tool"] == "fs_write")
            .expect("nazwa z manifestu narzędzia");
        assert_eq!(record["origin"], "native_replay");
        assert_eq!(record["recorded_status"], "done");
        let turns = a.h.core.turns_list(a.sid.clone()).await.unwrap();
        assert!(
            turns
                .turns
                .iter()
                .find(|t| t.id == answer)
                .unwrap()
                .tools
                .is_empty()
        );
        assert_single_write(&a).await;
    }
}

async fn assert_single_write(a: &common::agents::Agents) {
    let replay = a.h.core.agents_runs(a.sid.clone()).await.unwrap();
    assert_eq!(
        replay
            .iter()
            .flat_map(|r| &r.steps)
            .filter(|s| s.tool.as_deref() == Some("fs_write"))
            .count(),
        1,
        "odczyt nie uruchamia dodatkowego narzędzia"
    );
}

async fn restartable_agents() -> (
    common::agents::Agents,
    Arc<dyn accounts_hub_contract::SecretStore>,
) {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(HeadlessShell::default());
    let exec = Arc::new(platform_fake::FakeExec::new());
    let mut opts = common::options(Some(provider.clone()), shell.clone());
    let secrets = opts.secrets.clone().unwrap();
    opts.exec = Some(exec.clone());
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut a = common::agents::agents_on(core, provider, shell, dir).await;
    a.exec = exec;
    (a, secrets)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rejected_file_operation_is_not_reported_as_a_recorded_success() {
    use common::agents::{agents, run_turn};
    let mut a = agents().await;
    a.h.provider.push(Script::tool_call(
        FAKE_MODEL,
        "denied-1",
        "fs_write",
        &serde_json::json!({"path":"../outside-blocked.txt", "content":"private payload"}),
    ));
    a.h.provider
        .push(Script::text(FAKE_MODEL, &["Zapisałam mimo odmowy."]));
    run_turn(&mut a, "Sprawdź odmowę operacji poza katalogiem.").await;
    assert!(
        !a.workdir
            .parent()
            .unwrap()
            .join("outside-blocked.txt")
            .exists()
    );
    a.h.provider.push(Script::text(FAKE_MODEL, &["Meldunek."]));
    run_turn(&mut a, history_seed::QUERY).await;
    let request = a.h.provider.requests().last().cloned().unwrap();
    let facts = records(&request).expect("brak zapisu odrzuconego fs_write");
    assert_eq!(facts["counts"]["recorded_done"], 0);
    assert_eq!(facts["counts"]["error_or_denied"], 1);
    assert!(
        !serde_json::to_string(&facts)
            .unwrap()
            .contains("private payload")
    );
    assert!(
        request
            .messages
            .iter()
            .any(|m| m.visible_text().contains("Zapisałam mimo odmowy.")),
        "nie podmieniamy błędnej odpowiedzi w historii"
    );
}
