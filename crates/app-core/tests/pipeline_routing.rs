//! Etapy Pipeline wybierają dostawcę i klasę zadania niezależnie od Dyrygentki.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use app_core::dto::{SendOptions, SessionTemplate, SettingValue};
use app_core::ports::{BrainChoice, BrainError, BrainPort, BrainRequest, HeadlessShell};
use app_core::{AppCore, AppPaths};
use async_trait::async_trait;
use common::{ScriptedProvider, ends, options, until};
use providers_fake::{FAKE_MODEL, Script};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct StageBrain {
    main: Arc<ScriptedProvider>,
    stage: Arc<ScriptedProvider>,
    seen: Mutex<Vec<(String, String)>>,
    schema_counts: Mutex<Vec<(String, usize)>>,
}
#[async_trait]
impl BrainPort for StageBrain {
    async fn choose(&self, request: &BrainRequest) -> Result<BrainChoice, BrainError> {
        let class = request.task.clone().unwrap_or_default();
        let model = request
            .chat
            .as_ref()
            .map(|r| r.model.clone())
            .unwrap_or_default();
        self.seen.lock().unwrap().push((class.clone(), model));
        self.schema_counts.lock().unwrap().push((
            class.clone(),
            request.chat.as_ref().map_or(0, |r| r.tools.len()),
        ));
        Ok(BrainChoice {
            provider: if class == "code" {
                self.stage.clone()
            } else {
                self.main.clone()
            },
            provider_id: "fake".into(),
            provider_name: "Atrapa".into(),
            account: None,
            model: FAKE_MODEL.into(),
            context_window: Some(32_000),
            routed: false,
            route: None,
        })
    }
    fn keys_configured(&self) -> bool {
        true
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stage_uses_its_role_route_and_passes_pinned_model_to_router() {
    let dir = tempfile::tempdir().unwrap();
    let main = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let stage = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let brain = Arc::new(StageBrain {
        main: main.clone(),
        stage: stage.clone(),
        seen: Mutex::new(Vec::new()),
        schema_counts: Mutex::new(Vec::new()),
    });
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.brain = Some(brain.clone());
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
    main.push(Script::tool_call(FAKE_MODEL, "p", "run_pipeline", &json!({"input":"Build a screen", "steps":[{"role":"coder","persona":"delta","model":"fake:selected","goal":"Implement the screen"}]})));
    stage.push(Script::text(FAKE_MODEL, &["Kod z modelu etapu."]));
    main.push(Script::text(FAKE_MODEL, &["Gotowe."]));
    let sent = core
        .turns_send(
            sid,
            SendOptions {
                parent_id: None,
                text: "Uruchom pipeline".into(),
                addressed_to: Some("alfa".into()),
                profile: None,
                attachments: Vec::new(),
            },
        )
        .await
        .unwrap();
    until(&mut rx, ends(&sent.assistant_turn_id.unwrap())).await;
    assert_eq!(
        stage.requests().len(),
        1,
        "etap musi trafić do dostawcy z kolejki code"
    );
    assert!(
        brain
            .seen
            .lock()
            .unwrap()
            .contains(&("code".into(), "fake:selected".into()))
    );
    assert!(
        brain
            .schema_counts
            .lock()
            .unwrap()
            .iter()
            .filter(|(class, _)| class == "code")
            .all(|(_, count)| *count <= 24),
        "preflight etapu musi uwzględniać mały pakiet definicji"
    );
}
