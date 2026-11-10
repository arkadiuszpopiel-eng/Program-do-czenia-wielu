//! Odmowa rzeczywistego runtime trafia do tekstu rozmowy przez kompozycję aplikacji.
//! Model i narzędzie są skryptowane; nie jest to odbiór Windows ani rzeczywistego LLM.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use agent_runtime_contract::{AgentRuntime, MemCheckpointStore, StepStatus};
use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps};
use app_agents::{AgentSettings, SpecInput, final_text, run_spec};
use core_bus_contract::SessionId;
use personas_contract::{NameForms, PersonaId, builtin_personas, builtin_roles};
use providers_fake::{FAKE_MODEL, FakeProvider, Script};
use risk_classifier_contract::CommandOrigin;
use serde_json::json;
use tools_common_contract::{DenialReason, ScriptedTool, ToolOutcome};
use tools_fs_contract::{FsToolKind, manifest};

#[tokio::test]
async fn denied_write_is_visible_in_final_conversation_for_every_persona() {
    let mut personas = builtin_personas();
    let mut custom = personas[0].clone();
    custom.id = PersonaId::new("custom-coder");
    custom.name = "Nova".into();
    custom.forms = NameForms::uniform("Nova");
    custom.builtin = false;
    custom.wake_phrases = vec!["Hej Nova".into()];
    personas.push(custom);
    let dir = tempfile::tempdir().unwrap();
    for persona in personas {
        let agent = persona.id.to_string();
        let tool = Arc::new(ScriptedTool::new(manifest(FsToolKind::Write)));
        tool.push(ToolOutcome::denied(DenialReason::Policy, "zapis kodu"));
        let provider = FakeProvider::new("fake");
        provider.push_script(Script::tool_call(
            FAKE_MODEL,
            "write",
            "fs_write",
            &json!({"path":"private-fixture.rs","content":"fn main() {}"}),
        ));
        // Odpowiedź modelu sama nie opisuje odmowy; aplikacja zachowuje fakt wykonania.
        provider.push_script(Script::text(FAKE_MODEL, &["Sprawdź kod programu."]));
        let runtime = Runtime::new(RuntimeDeps {
            provider: Arc::new(provider.clone()),
            tools: vec![tool.clone()],
            checkpoints: Arc::new(MemCheckpointStore::default()),
            bus: None,
            config: RuntimeConfig::default(),
        });
        let spec = run_spec(
            SpecInput {
                session: SessionId::new(format!("denied-{agent}")),
                persona,
                roles: builtin_roles()
                    .into_iter()
                    .filter(|r| r.id.as_str() == "operator")
                    .collect(),
                goal: "Popraw i sprawdź kod".into(),
                origin: CommandOrigin::UserText,
                model: FAKE_MODEL.into(),
                tools: vec!["fs_write".into()],
                workdir: dir.path().to_string_lossy().into_owned(),
                history: Vec::new(),
                environment: None,
            },
            &AgentSettings {
                verify: false,
                ..AgentSettings::default()
            },
            40_000,
            true,
        );
        assert_eq!(spec.agent.as_str(), agent);
        let run = runtime.start(spec).await.unwrap();
        let outcome = runtime.wait(&run).await.unwrap();
        let projected = final_text(&outcome);
        assert!(projected.text.contains("Odmowa narzędzia"), "{agent}");
        assert!(
            projected.text.contains("nie wykonano 1 operacji"),
            "{agent}"
        );
        assert!(!projected.text.contains("private-fixture.rs"), "{agent}");
        assert!(projected.error.is_none(), "{agent}");
        let report = runtime.report(&run).unwrap();
        assert!(report.done.is_empty(), "{agent}");
        assert_eq!(report.not_done.len(), 1, "{agent}");
        assert_eq!(report.not_done[0].status, StepStatus::Denied, "{agent}");
        assert_eq!(tool.calls().len(), 1, "{agent}");
        assert_eq!(provider.requests().len(), 2, "{agent}");
    }
}
