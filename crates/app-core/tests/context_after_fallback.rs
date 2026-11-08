//! Licznik kontekstu po fallbacku Routera: okno modelu, który naprawdę odpowiada, a nie modelu
//! z decyzji przed wysłaniem (przegląd wymiany modeli, 1.11).
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use app_core::dto::{SessionTemplate, SettingValue};
use app_core::ports::{
    BrainChoice, BrainError, BrainPort, BrainRequest, BrainTarget, HeadlessShell,
};
use app_core::{AppCore, AppPaths};
use async_trait::async_trait;
use common::{ScriptedProvider, ends, options, send, until};
use providers_fake::{FAKE_MODEL, Script};
use std::sync::Arc;
use std::time::Duration;

/// Decyzja wskazuje model z oknem 200 000; odpowiada (po fallbacku) model lokalny z 8 192.
struct FallbackBrain(Arc<ScriptedProvider>);

#[async_trait]
impl BrainPort for FallbackBrain {
    async fn choose(&self, _request: &BrainRequest) -> Result<BrainChoice, BrainError> {
        Ok(BrainChoice {
            provider: self.0.clone(),
            provider_id: "router".into(),
            provider_name: "Router".into(),
            account: None,
            model: "auto".into(),
            context_window: Some(200_000),
            routed: true,
            route: None,
        })
    }
    fn keys_configured(&self) -> bool {
        true
    }
    fn target(&self, model: &str) -> Option<BrainTarget> {
        (model == FAKE_MODEL).then(|| BrainTarget {
            provider_id: "local".into(),
            provider_name: "Model lokalny".into(),
            account: None,
            model: FAKE_MODEL.into(),
            local: true,
            context_window: Some(8_192),
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn context_meter_follows_the_model_that_answered() {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.brain = Some(Arc::new(FallbackBrain(provider.clone())));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
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
    let mut rx = core.subscribe_events();
    provider.push(Script::text(FAKE_MODEL, &["Odpowiedź modelu lokalnego."]));
    let sent = core
        .turns_send(sid.clone(), send("Cześć", None))
        .await
        .unwrap();
    until(&mut rx, ends(&sent.assistant_turn_id.unwrap())).await;
    let summary = core.costs_summary(Some(sid)).await.unwrap();
    assert_eq!(summary.context.max_tokens, 8_192);
}
