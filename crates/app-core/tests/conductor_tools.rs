//! Narzędzia w zwykłej rozmowie (zgłoszenie właściciela 2026-10-20: Alfa odpowiadała „nie mam
//! dostępu do komputera”): rozmowa bez wybranego katalogu roboczego używa katalogu sesji, a Alfa
//! jako Dyrygentka (bez narzędzi) zostaje zwykłym czatem, ale wie, która agentka obsady działa
//! na komputerze i jak ją wywołać. Jawne „bez katalogu” — zwykły czat bez tej informacji.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use app_core::dto::{SendOptions, SessionTemplate, SettingValue, WorkdirChoice};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, ends, options, until};
use providers_fake::{FAKE_MODEL, Script};

struct Setup {
    core: AppCore,
    provider: Arc<ScriptedProvider>,
    sid: String,
    rx: tokio::sync::broadcast::Receiver<app_core::EventBatch>,
    _dir: tempfile::TempDir,
}

/// Rdzeń z sesją, w której nikt nie wybierał katalogu roboczego.
async fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(HeadlessShell::default());
    let mut opts = options(Some(provider.clone()), shell);
    opts.approval_timeout = Some(Duration::from_secs(1));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let rx = core.subscribe_events();
    core.settings_set(
        "agents.verify_before_done".into(),
        SettingValue::Bool(false),
    )
    .await
    .unwrap();
    let sid = core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    Setup {
        core,
        provider,
        sid,
        rx,
        _dir: dir,
    }
}

fn message(text: &str, to: Option<&str>) -> SendOptions {
    SendOptions {
        parent_id: None,
        text: text.into(),
        addressed_to: to.map(Into::into),
        profile: None,
        attachments: Vec::new(),
    }
}

async fn ask(s: &mut Setup, text: &str, to: Option<&str>) -> String {
    let sent = s
        .core
        .turns_send(s.sid.clone(), message(text, to))
        .await
        .unwrap();
    let turn = sent.assistant_turn_id.unwrap();
    until(&mut s.rx, ends(&turn)).await;
    let snapshot = s.core.turns_list(s.sid.clone()).await.unwrap();
    snapshot
        .turns
        .iter()
        .find(|t| t.id == turn)
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn session_without_chosen_workdir_still_gives_tools() {
    let mut s = setup().await;
    assert_eq!(
        s.core.sessions_workdir(s.sid.clone()).await.unwrap().path,
        None,
        "nikt nie wybierał katalogu"
    );
    s.provider
        .push(Script::text(FAKE_MODEL, &["Gotowe, sprawdziłam."]));
    let reply = ask(&mut s, "Delta, sprawdź katalog.", Some("delta")).await;
    assert_eq!(reply, "Gotowe, sprawdziłam.");
    let runs = s.core.agents_runs(s.sid.clone()).await.unwrap();
    assert_eq!(runs.len(), 1, "odpowiedź przebiegiem agentki z narzędziami");
    let request = s.provider.requests().last().cloned().unwrap();
    assert!(!request.tools.is_empty(), "żądanie z narzędziami");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn conductor_knows_who_works_on_the_computer() {
    let mut s = setup().await;
    s.provider
        .push(Script::text(FAKE_MODEL, &["Zrobi to Delta."]));
    let reply = ask(&mut s, "Jak teraz z dostępem do komputera?", None).await;
    assert_eq!(reply, "Zrobi to Delta.");
    // Zwykłe pytanie bez wywołania delegate_task zostaje czatem bez przebiegu w Replay.
    assert!(s.core.agents_runs(s.sid.clone()).await.unwrap().is_empty());
    let request = s.provider.requests().last().cloned().unwrap();
    assert_eq!(
        request
            .tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        vec!["delegate_task", "run_pipeline"],
        "Dyrygentka ma narzędzia delegate_task i run_pipeline do rozdzielania pracy"
    );
    let system = request.system.unwrap_or_default();
    assert!(
        system.contains("na komputerze użytkownika działa: Delta"),
        "{system}"
    );
    assert!(
        system.contains("delegate_task") && !system.contains("„Delta, …”"),
        "{system}"
    );
    // Poziom autonomii rozmowy (Broker w procesie: domyślne L3) — bez zapowiadania zgód na wszystko.
    // Opis z tabeli reguł Brokera: na L3 pyta m.in. o host spoza listy, nie o każdą zmianę.
    assert!(
        system.contains("Poziom autonomii tej rozmowy: L3")
            && system.contains("host spoza egress-allowlisty")
            && system.contains("operacja administracyjna")
            && !system.contains("każda zmiana wymaga"),
        "{system}"
    );
    s.core
        .permissions_request_level(app_core::dto::AutonomyLevel::L1, Some(s.sid.clone()))
        .await
        .unwrap();
    s.provider
        .push(Script::text(FAKE_MODEL, &["Delta zapyta o zgodę."]));
    ask(&mut s, "A teraz?", None).await;
    let system = s.provider.requests().last().cloned().unwrap().system;
    assert!(
        system
            .unwrap_or_default()
            .contains("Poziom autonomii tej rozmowy: L1 — Broker pyta właściciela o zgodę"),
        "poziom sesji po obniżeniu"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn conductor_dispatches_tasks_across_agents_and_switches_as_needed() {
    let mut s = setup().await;
    let chosen = s
        .core
        .sessions_choose_workdir(s.sid.clone(), WorkdirChoice::Default)
        .await
        .unwrap();
    let workdir = std::path::PathBuf::from(chosen.path.unwrap());
    let target_file = workdir.join("wynik.txt");

    // 1. Alfa (Dyrygentka) zleca analizę Gamie (researcher).
    s.provider.push(Script::tool_call(
        FAKE_MODEL,
        "d1",
        "delegate_task",
        &serde_json::json!({
            "role": "researcher",
            "persona": "gama",
            "goal": "Check project structure"
        }),
    ));
    // 2. Gama wykonuje swoje zadanie i raportuje wynik.
    s.provider
        .push(Script::text(FAKE_MODEL, &["Struktura sprawdzona."]));
    // 3. Alfa przełącza zadanie na Deltę (coder), żeby wykonała zapis w repozytorium.
    s.provider.push(Script::tool_call(
        FAKE_MODEL,
        "d2",
        "delegate_task",
        &serde_json::json!({
            "role": "coder",
            "persona": "delta",
            "goal": "Write wynik.txt"
        }),
    ));
    // 4. Delta (jako Koderka z dostępem do narzędzi fs_*) zapisuje plik wynik.txt.
    s.provider.push(Script::tool_call(
        FAKE_MODEL,
        "w1",
        "fs_write",
        &serde_json::json!({
            "path": target_file.to_string_lossy(),
            "content": "plan-ok",
            "mode": "create"
        }),
    ));
    s.provider
        .push(Script::text(FAKE_MODEL, &["Plik wynik.txt zapisany."]));
    // 5. Alfa podsumowuje pracę obu agentek właścicielowi.
    s.provider.push(Script::text(
        FAKE_MODEL,
        &["Plan wykonany: Gama przeanalizowała projekt, a Delta zapisała wynik.txt."],
    ));

    let reply = ask(&mut s, "Wykonaj plan", None).await;
    assert_eq!(
        reply,
        "Plan wykonany: Gama przeanalizowała projekt, a Delta zapisała wynik.txt."
    );
    assert_eq!(
        std::fs::read_to_string(&target_file).unwrap(),
        "plan-ok",
        "Delta wykonała zapis pliku w katalogu roboczym"
    );
    let runs = s.core.agents_runs(s.sid.clone()).await.unwrap();
    let agents: Vec<&str> = runs.iter().map(|r| r.run.agent.as_str()).collect();
    assert!(
        agents.contains(&"alfa") && agents.contains(&"gama") && agents.contains(&"delta"),
        "przebiegi Alfy, Gamy i Delty w Replay: {agents:?}"
    );
    let turns = s.core.turns_list(s.sid.clone()).await.unwrap().turns;
    let last_turn = turns.last().unwrap();
    let labels: Vec<&str> = last_turn.tools.iter().map(|t| t.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.contains("Zlecenie → Gama"))
            && labels.iter().any(|l| l.contains("Zlecenie → Delta"))
            && labels.iter().any(|l| l.contains("delta: zapisano 1 plik")),
        "kroki delegacji i narzędzi podrzędnych agentek w turze: {labels:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn conductor_auto_delegates_when_model_replies_with_manual_handoff_text() {
    let mut s = setup().await;
    // Nawet jeśli w starej sesji model odruchowo odpisze dawną formułką odsyłającą użytkownika
    // do ręcznego wpisania „Delta, …”, Dyrygentka automatycznie zamienia to w delegację dla Delty.
    s.provider.push(Script::text(
        FAKE_MODEL,
        &[
            "Jasne — to wymaga pracy w repozytorium: pomiarów, zmian, testów i weryfikacji. ",
            "Zrobi to Delta. Napisz „Delta, wykonaj pięciopunktowy plan ulepszeń aplikacji” ",
            "albo wybierz Deltę w polu adresatki.",
        ],
    ));
    // Delta wykonuje zadanie.
    s.provider.push(Script::text(
        FAKE_MODEL,
        &["Wykonałam wszystkie 5 punktów planu."],
    ));
    // Alfa podsumowuje wykonanie zamiast odsyłać użytkownika.
    s.provider.push(Script::text(
        FAKE_MODEL,
        &["Gotowe — Delta wykonała cały plan."],
    ));

    let reply = ask(&mut s, "wykonaj plan", None).await;
    assert_eq!(reply, "Gotowe — Delta wykonała cały plan.");
    let runs = s.core.agents_runs(s.sid.clone()).await.unwrap();
    let agents: Vec<&str> = runs.iter().map(|r| r.run.agent.as_str()).collect();
    assert!(
        agents.contains(&"alfa") && agents.contains(&"delta"),
        "automatyczna delegacja uruchomiła Deltę: {agents:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn conductor_runs_model_pipeline_without_user_in_between() {
    let mut s = setup().await;
    s.core
        .sessions_choose_workdir(s.sid.clone(), WorkdirChoice::Default)
        .await
        .unwrap();
    // Dyrygentka uruchamia pipeline: styl → architektura → kod (każdy etap na innym modelu).
    s.provider.push(Script::tool_call(
        FAKE_MODEL,
        "p1",
        "run_pipeline",
        &serde_json::json!({
            "preset": "ui-design",
            "input": "Szkic: formularz logowania",
            "models": ["fake:astra", "fake:opus", "fake:sol"]
        }),
    ));
    s.provider
        .push(Script::text(FAKE_MODEL, &["WYTYCZNE-WIZUALNE"]));
    s.provider
        .push(Script::text(FAKE_MODEL, &["ARCHITEKTURA-KOMPONENTOW"]));
    s.provider.push(Script::text(FAKE_MODEL, &["KOD-TAILWIND"]));
    s.provider.push(Script::text(
        FAKE_MODEL,
        &["Pipeline gotowy: KOD-TAILWIND."],
    ));

    let reply = ask(&mut s, "Zrób UI z tego szkicu przez pipeline", None).await;
    assert_eq!(reply, "Pipeline gotowy: KOD-TAILWIND.");
    let requests = s.provider.requests();
    let text_of = |i: usize| {
        requests[i]
            .messages
            .iter()
            .map(|m| m.visible_text())
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(
        text_of(2).contains("WYTYCZNE-WIZUALNE"),
        "etap 2 dostał wynik etapu 1"
    );
    assert!(
        text_of(3).contains("ARCHITEKTURA-KOMPONENTOW"),
        "etap 3 dostał wynik etapu 2"
    );
    let turns = s.core.turns_list(s.sid.clone()).await.unwrap().turns;
    let labels: Vec<&str> = turns
        .last()
        .unwrap()
        .tools
        .iter()
        .map(|t| t.label.as_str())
        .collect();
    assert!(
        labels.iter().any(|l| l.contains("Pipeline 1/3"))
            && labels.iter().any(|l| l.contains("Pipeline 3/3")),
        "etapy pipeline'u widoczne w turze: {labels:?}"
    );
}
