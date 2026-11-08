//! Styl odpowiedzi i swoboda (P4) na prawdziwym rdzeniu: ustawienia trafiają do żądania —
//! zdanie stylu w prompcie systemowym, temperatura w parametrach; „auto” niczego nie zmienia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use app_core::dto::{SessionTemplate, SettingValue};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths};
use common::*;
use providers_fake::{FAKE_MODEL, Script};

async fn ask(core: &AppCore, provider: &ScriptedProvider, sid: &str, before: usize) {
    core.turns_send(sid.to_owned(), send("Opowiedz o Wiśle", None))
        .await
        .unwrap();
    for _ in 0..300 {
        if provider.requests().len() > before {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    core.turns_stop(sid.to_owned()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn style_and_temperature_reach_the_request() {
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
    ask(&core, &provider, &sid, 0).await;
    let plain = provider.requests()[0].clone();
    assert_eq!(plain.params.temperature, None, "auto — domyślna modelu");
    assert!(
        !plain
            .system
            .clone()
            .unwrap_or_default()
            .contains("Styl odpowiedzi")
    );

    core.settings_set(
        "chat.reply_style".into(),
        SettingValue::Text("concise".into()),
    )
    .await
    .unwrap();
    core.settings_set(
        "chat.temperature".into(),
        SettingValue::Text("precise".into()),
    )
    .await
    .unwrap();
    ask(&core, &provider, &sid, 1).await;
    let styled = provider.requests()[1].clone();
    assert_eq!(styled.params.temperature, Some(0.2));
    assert!(
        styled.system.unwrap_or_default().contains("zwięźle"),
        "zdanie stylu w prompcie systemowym"
    );
}

/// Raport 32: przebieg agentki (Delta z narzędziami) pomijał styl i temperaturę z ustawień —
/// docierały tylko do zwykłej rozmowy.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn style_and_temperature_reach_agent_runs() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let opts = options(Some(provider.clone()), Arc::new(HeadlessShell::default()));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut rx = core.subscribe_events();
    for (key, value) in [
        ("chat.reply_style", "concise"),
        ("chat.temperature", "precise"),
    ] {
        core.settings_set(key.into(), SettingValue::Text(value.into()))
            .await
            .unwrap();
    }
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
    provider.push(Script::text(FAKE_MODEL, &["Gotowe, sprawdziłam."]));
    let mut message = send("Delta, sprawdź katalog.", None);
    message.addressed_to = Some("delta".into());
    let sent = core.turns_send(sid.clone(), message).await.unwrap();
    until(&mut rx, ends(&sent.assistant_turn_id.unwrap())).await;
    assert_eq!(
        core.agents_runs(sid.clone()).await.unwrap().len(),
        1,
        "odpowiedź przebiegiem agentki"
    );
    let request = provider.requests().last().cloned().unwrap();
    assert!(!request.tools.is_empty(), "żądanie przebiegu z narzędziami");
    assert_eq!(request.params.temperature, Some(0.2));
    assert!(
        request.system.unwrap_or_default().contains("zwięźle"),
        "zdanie stylu w prompcie przebiegu"
    );
}
