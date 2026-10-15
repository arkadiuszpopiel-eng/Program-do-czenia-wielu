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
