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
    // Rozmowa z Dyrygentką zostaje zwykłym czatem (strumień, głos, styl) — bez przebiegu.
    assert!(s.core.agents_runs(s.sid.clone()).await.unwrap().is_empty());
    let request = s.provider.requests().last().cloned().unwrap();
    assert!(request.tools.is_empty());
    let system = request.system.unwrap_or_default();
    assert!(
        system.contains("na komputerze użytkownika działa: Delta"),
        "{system}"
    );
    assert!(system.contains("„Delta, …”"), "{system}");
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
async fn explicit_no_workdir_keeps_plain_chat() {
    let mut s = setup().await;
    let none = s
        .core
        .sessions_choose_workdir(s.sid.clone(), WorkdirChoice::None)
        .await
        .unwrap();
    assert_eq!(none.path, None);
    let reply = ask(&mut s, "Co słychać?", None).await;
    assert!(reply.starts_with("Echo:"), "{reply}");
    assert!(s.core.agents_runs(s.sid.clone()).await.unwrap().is_empty());
    let system = s.provider.requests().last().cloned().unwrap().system;
    assert!(
        !system
            .unwrap_or_default()
            .contains("na komputerze użytkownika"),
        "bez katalogu nikt nie działa na komputerze"
    );
}
