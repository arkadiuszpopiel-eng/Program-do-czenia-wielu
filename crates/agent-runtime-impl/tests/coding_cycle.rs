//! Skryptowany model, ale rzeczywiste rustc, pliki, binaria i testy w prywatnym tempdir.
//! To test integracji pętli, nie pomiar inteligencji modelu ani odbiór Windows/UAC.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::{path::Path, process::Command, sync::Arc};

use agent_runtime_contract::{AgentRuntime, RunOutcome};
use agent_runtime_impl::RuntimeConfig;
use async_trait::async_trait;
use common::{answer, call, spec, world_with};
use personas_contract::{NameForms, Persona, PersonaId, builtin_personas, builtin_roles};
use providers_contract::{ContentBlock, ToolResultPart};
use risk_classifier_contract::Reversibility;
use safety_broker_contract::TaintSource;
use serde_json::{Value, json};
use tempfile::TempDir;
use tools_common_contract::{Tool, ToolCtx, ToolManifest, ToolOutcome};

const BROKEN: &str = "fn answer() -> u32 { \"42\" }\nfn main() { println!(\"{}\", answer()); }\n#[test] fn answer_is_42() { assert_eq!(answer(), 42); }\n";

struct CodingTool {
    dir: Arc<TempDir>,
    manifest: ToolManifest,
}

impl CodingTool {
    fn new(dir: Arc<TempDir>, review: bool) -> Self {
        Self {
            dir,
            manifest: ToolManifest {
                name: if review {
                    "coding_review"
                } else {
                    "coding_probe"
                }
                .into(),
                id: "test.coding".into(),
                title: "Próba kodowania".into(),
                description: "Testowy adapter kompilatora ograniczony do prywatnego tempdir."
                    .into(),
                input_schema: json!({"type":"object","properties":{"action":{"type":"string"}},"required":["action"],"additionalProperties":false}),
                output_schema: json!({"type":"object"}),
                reversible: Reversibility::Scoped,
                capabilities: vec![if review { "fs.read" } else { "shell.exec" }.into()],
                groups: vec!["shell".into()],
                mutating: !review,
                untrusted_output: Some(TaintSource::File),
            },
        }
    }
}

fn execute(dir: &Path, program: &Path, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    format!(
        "exit={}\n{}{}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[async_trait]
impl Tool for CodingTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: Value, ctx: &ToolCtx) -> ToolOutcome {
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled("próba kodowania");
        }
        let dir = self.dir.path();
        let action = args["action"].as_str().unwrap();
        assert_eq!(action == "review", !self.manifest.mutating);
        let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let executable = if cfg!(windows) {
            "program.exe"
        } else {
            "program"
        };
        let tests = if cfg!(windows) {
            "checks.exe"
        } else {
            "checks"
        };
        let text = match action {
            "compile" => execute(dir, Path::new(&compiler), &["main.rs", "-o", executable]),
            "repair" => {
                assert!(self.manifest.mutating);
                std::fs::write(dir.join("main.rs"), BROKEN.replace("\"42\"", "42")).unwrap();
                "Zapisano poprawkę typu w main.rs".into()
            }
            "test" => {
                let build = execute(
                    dir,
                    Path::new(&compiler),
                    &["--test", "main.rs", "-o", tests],
                );
                assert!(build.starts_with("exit=0\n"), "{build}");
                execute(dir, &dir.join(tests), &[])
            }
            "run" => {
                let build = execute(dir, Path::new(&compiler), &["main.rs", "-o", executable]);
                assert!(build.starts_with("exit=0\n"), "{build}");
                execute(dir, &dir.join(executable), &[])
            }
            "review" => {
                assert!(!self.manifest.mutating);
                ["main.rs", "compile.log", "test.log", "run.log"]
                    .iter()
                    .map(|name| {
                        format!(
                            "{name}:\n{}",
                            std::fs::read_to_string(dir.join(name)).unwrap()
                        )
                    })
                    .collect()
            }
            _ => panic!("akcja spoza zamkniętego scenariusza testowego"),
        };
        if self.manifest.mutating {
            std::fs::write(dir.join(format!("{action}.log")), &text).unwrap();
        }
        ToolOutcome::ok(text, json!({})).untrusted(TaintSource::File)
    }
}

fn observed(request: &providers_contract::ChatRequest) -> String {
    request
        .messages
        .iter()
        .flat_map(|m| &m.content)
        .filter_map(|c| match c {
            ContentBlock::ToolResult(r) => Some(
                r.content
                    .iter()
                    .filter_map(|p| match p {
                        ToolResultPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect()
}

async fn coding_cycle(persona: Persona) {
    let name = persona.name.clone();
    let dir = Arc::new(tempfile::tempdir().unwrap());
    std::fs::write(dir.path().join("main.rs"), BROKEN).unwrap();
    let w = world_with(
        RuntimeConfig::default(),
        vec![
            Arc::new(CodingTool::new(dir.clone(), false)),
            Arc::new(CodingTool::new(dir.clone(), true)),
        ],
    );
    for (id, action) in ["compile", "repair", "test", "run"].iter().enumerate() {
        w.provider.push_script(call(
            &format!("step-{id}"),
            "coding_probe",
            json!({"action":action}),
        ));
    }
    w.provider
        .push_script(answer("Poprawiłam typ; test i program działają."));
    w.provider
        .push_script(call("review", "coding_review", json!({"action":"review"})));
    w.provider
        .push_script(answer("WERYFIKACJA: OK — kod, test i wynik są spójne."));
    let mut s = spec();
    s.agent = core_bus_contract::AgentId::new(persona.id.as_str());
    s.persona = persona;
    s.goal = "Napraw, skompiluj, przetestuj i uruchom program; sprawdź wynik.".into();
    s.roles = builtin_roles()
        .into_iter()
        .filter(|r| r.id.as_str() == "coder")
        .collect();
    s.tools = vec!["coding_probe".into(), "coding_review".into()];
    s.workdir = Some(dir.path().to_string_lossy().into_owned());
    s.verify = true;
    let run = w.runtime.start(s).await.unwrap();
    assert!(matches!(
        w.runtime.wait(&run).await.unwrap(),
        RunOutcome::Completed {
            verified: Some(true),
            ..
        }
    ));
    let requests = w.provider.requests();
    assert!(requests[0].system.as_ref().unwrap().contains(&name));
    assert!(observed(&requests[1]).contains("error[E0308]"));
    assert!(observed(&requests[1]).contains("exit=1"));
    assert!(observed(&requests[3]).contains("1 passed; 0 failed"));
    assert!(observed(&requests[4]).contains("exit=0\n42\n"));
    assert!(requests[5].tools.iter().all(|t| t.name != "coding_probe"));
    assert!(observed(&requests[6]).contains("run.log:\nexit=0\n42\n"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("main.rs")).unwrap(),
        BROKEN.replace("\"42\"", "42")
    );
}

async fn builtin_coding_cycle(id: &str) {
    coding_cycle(
        builtin_personas()
            .into_iter()
            .find(|p| p.id.as_str() == id)
            .unwrap(),
    )
    .await;
}

#[tokio::test]
async fn alfa_compiles_repairs_tests_runs_and_reviews() {
    builtin_coding_cycle("alfa").await;
}

#[tokio::test]
async fn beta_compiles_repairs_tests_runs_and_reviews() {
    builtin_coding_cycle("beta").await;
}

#[tokio::test]
async fn gama_compiles_repairs_tests_runs_and_reviews() {
    builtin_coding_cycle("gama").await;
}

#[tokio::test]
async fn delta_compiles_repairs_tests_runs_and_reviews() {
    builtin_coding_cycle("delta").await;
}

#[tokio::test]
async fn custom_persona_compiles_repairs_tests_runs_and_reviews() {
    let mut persona = builtin_personas().remove(0);
    persona.id = PersonaId::new("custom-coder");
    persona.name = "Nova".into();
    persona.forms = NameForms::uniform("Nova");
    persona.builtin = false;
    persona.wake_phrases = vec!["Hej Nova".into()];
    coding_cycle(persona).await;
}
