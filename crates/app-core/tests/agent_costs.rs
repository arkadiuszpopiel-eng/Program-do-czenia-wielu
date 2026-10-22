//! Przegląd 2026-10-22: koszt przebiegu agentki przez Router zapisywał się pod dostawcą „router”,
//! więc limit konta (dostawcy) go nie widział — limit miesięczny tak. Teraz każda tura trafia do
//! dziennika kosztów pod dostawcą, który faktycznie odpowiedział.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use app_core::dto::{SendOptions, SessionTemplate, SettingValue, WorkdirChoice};
use app_core::ports::HeadlessShell;
use app_core::{AppCore, AppPaths, RouteKind};
use common::*;
use providers_contract::{ModelProvider, Pricing};
use providers_fake::{FAKE_MODEL, FakeProvider, Script};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn agent_run_cost_is_recorded_under_the_provider_that_answered() {
    let dir = tempfile::tempdir().unwrap();
    let openai = FakeProvider::new("openai")
        .with_default_script(Script::text(FAKE_MODEL, &["Gotowe."]))
        .with_pricing(
            FAKE_MODEL,
            Pricing {
                input_per_mtok_usd: 3.0,
                output_per_mtok_usd: 15.0,
                cache_read_per_mtok_usd: None,
                cache_write_per_mtok_usd: None,
            },
        );
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.providers = vec![(
        Arc::new(openai.clone()) as Arc<dyn ModelProvider>,
        RouteKind::Api,
    )];
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut rx = core.subscribe_events();
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
    core.sessions_choose_workdir(sid.clone(), WorkdirChoice::Default)
        .await
        .unwrap();
    let sent = core
        .turns_send(
            sid.clone(),
            SendOptions {
                parent_id: None,
                text: "Delta, sprawdź folder.".into(),
                addressed_to: Some("delta".into()),
                profile: None,
                attachments: Vec::new(),
            },
        )
        .await
        .unwrap();
    let turn = sent.assistant_turn_id.unwrap();
    until(&mut rx, ends(&turn)).await;
    assert!(
        !openai.calls().is_empty(),
        "agentka odpowiadała przez Router"
    );

    let log = std::fs::read_to_string(AppPaths::under(dir.path()).state().join("costs.ndjson"))
        .expect("dziennik kosztów");
    let records: Vec<serde_json::Value> = log
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(!records.is_empty(), "koszt przebiegu zapisany");
    for r in &records {
        assert_eq!(r["provider"], "openai", "{r}");
        assert_eq!(r["model"], FAKE_MODEL, "{r}");
    }
    assert!(
        records
            .iter()
            .any(|r| r["micro_usd"].as_u64().unwrap_or(0) > 0),
        "{records:?}"
    );
}
