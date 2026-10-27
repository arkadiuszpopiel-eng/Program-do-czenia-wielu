//! Router plus actual HTTP codec: a model absent from the enumerated capabilities must retain delegation.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;
use app_core::dto::{SessionTemplate, SettingValue};
use app_core::ports::{BrainChoice, BrainError, BrainPort, BrainRequest, HeadlessShell, RouteNote};
use app_core::{AppCore, AppPaths};
use async_trait::async_trait;
use common::{ends, options, send, until};
use providers_api_impl::{AuthScheme, HttpConfig, OpenAiOptions, OpenAiProvider, ProviderProfile};
use providers_contract::{ModelCapabilities, ModelProvider, StaticKey};
use router_contract::{Candidate, Constraints, RouteKind, TaskClass};
use router_impl::{RoutedProvider, RouterCore};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
};
struct Brain(Arc<RouterCore>);
#[async_trait]
impl BrainPort for Brain {
    async fn choose(&self, r: &BrainRequest) -> Result<BrainChoice, BrainError> {
        let class = TaskClass::Conversation;
        let constraints = r
            .chat
            .as_ref()
            .map(|c| Constraints::from_request(class, c))
            .unwrap_or_default();
        let decision = self
            .0
            .decide(class, &constraints, r.chat.as_ref())
            .map_err(|e| BrainError::Provider(e.to_string()))?;
        Ok(BrainChoice {
            provider: Arc::new(RoutedProvider::new(self.0.clone(), class)),
            provider_id: "router".into(),
            provider_name: "Router".into(),
            account: None,
            model: "auto".into(),
            context_window: None,
            routed: true,
            route: Some(RouteNote {
                chosen: decision.chosen.qualified(),
                fallbacks: decision
                    .fallbacks
                    .iter()
                    .map(Candidate::qualified)
                    .collect(),
                ..Default::default()
            }),
        })
    }
    fn keys_configured(&self) -> bool {
        true
    }
}
fn server() -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    std::thread::spawn(move || {
        for incoming in listener.incoming() {
            let Ok(mut stream) = incoming else { break };
            let mut bytes = Vec::new();
            let mut buf = [0; 4096];
            let head = loop {
                if let Some(n) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break n + 4;
                }
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    return;
                }
                bytes.extend_from_slice(&buf[..n]);
            };
            let length = String::from_utf8_lossy(&bytes[..head])
                .lines()
                .find_map(|l| {
                    l.split_once(':')
                        .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                        .map(|(_, v)| v.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            while bytes.len() < head + length {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    return;
                }
                bytes.extend_from_slice(&buf[..n]);
            }
            let request: Value = serde_json::from_slice(&bytes[head..head + length]).unwrap();
            let index = {
                let mut seen = seen.lock().unwrap();
                seen.push(request.clone());
                seen.len()
            };
            let call = match index {
                1 => Some((
                    "delegate_task",
                    json!({"persona":"delta","role":"coder","goal":"Sprawdź katalog przez fs_list, odczytaj README.md i podaj wynik."}),
                )),
                2 => Some(("fs_list", json!({"path":"."}))),
                3 => Some(("fs_read", json!({"path":"README.md"}))),
                _ => None,
            };
            let (delta, finish) = if let Some((name, args)) = call {
                (
                    json!({"role":"assistant","tool_calls":[{"index":0,"id":format!("call-{index}"),"type":"function","function":{"name":name,"arguments":args.to_string()}}]}),
                    "tool_calls",
                )
            } else {
                (
                    json!({"role":"assistant","content":"Przegląd wykonany."}),
                    "stop",
                )
            };
            let model = request["model"].as_str().unwrap();
            let chunk = json!({"id":"x","model":model,"choices":[{"index":0,"delta":delta,"finish_reason":null}]});
            let done = json!({"id":"x","model":model,"choices":[{"index":0,"delta":{},"finish_reason":finish}]});
            let body = format!("data: {chunk}\n\ndata: {done}\n\ndata: [DONE]\n\n");
            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(reply.as_bytes()).unwrap();
        }
    });
    (base, requests)
}
async fn run_case(text_first: bool) {
    let (base, seen) = server();
    let mut profile = ProviderProfile::new("openai");
    profile.default_model = Some("new-model".into());
    if text_first {
        profile
            .models
            .insert("chat-only".into(), ModelCapabilities::default());
    }
    let provider = Arc::new(
        OpenAiProvider::with_fallback(
            profile,
            HttpConfig::new(&base, AuthScheme::Bearer),
            Arc::new(StaticKey::new("local-test-not-a-real-key")),
            OpenAiOptions::compatible(),
            ModelCapabilities {
                tools: true,
                ..Default::default()
            },
        )
        .unwrap(),
    );
    assert!(!provider.capabilities().models.contains_key("new-model"));
    let router = Arc::new(RouterCore::new(None, None));
    router.register(provider, RouteKind::Api);
    let mut queue = vec![Candidate::new("openai", "new-model")];
    if text_first {
        queue.insert(0, Candidate::new("openai", "chat-only"));
    }
    router.set_global_queues([(TaskClass::Conversation, queue)].into());
    let dir = tempfile::tempdir().unwrap();
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.brain = Some(Arc::new(Brain(router)));
    let c = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    c.settings_set(
        "agents.verify_before_done".into(),
        SettingValue::Bool(false),
    )
    .await
    .unwrap();
    let sid = c.sessions_create(SessionTemplate::Empty).await.unwrap().id;
    let project_dir = dir.path().join("selected-project");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::write(project_dir.join("README.md"), "selected-project-marker").unwrap();
    c.projects_save(
        "Tłumacz AI".into(),
        Some(project_dir.to_string_lossy().into_owned()),
    )
    .await
    .unwrap();
    c.sessions_set_project(sid.clone(), Some("Tłumacz AI".into()))
        .await
        .unwrap();
    let mut rx = c.subscribe_events();
    let turn = c
        .turns_send(
            sid.clone(),
            send("Analiza programu i 5 zalecanych ulepszen", None),
        )
        .await
        .unwrap()
        .assistant_turn_id
        .unwrap();
    until(&mut rx, ends(&turn)).await;
    // Kopia, nie blokada — dalej jest `await` (clippy: await_holding_lock).
    let requests = seen.lock().unwrap().clone();
    assert_eq!(requests[0]["model"], "new-model");
    for index in [0, 1] {
        let messages = requests[index]["messages"].as_array().unwrap();
        let prompt = messages
            .iter()
            .filter_map(|m| m["content"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            prompt.contains("\"project\":\"Tłumacz AI\""),
            "request {index} is missing selected project"
        );
        assert!(
            prompt.contains(&serde_json::to_string(&project_dir.to_string_lossy()).unwrap()),
            "request {index} is missing project directory"
        );
    }
    assert!(
        requests[2]["messages"].to_string().contains("README.md"),
        "Delta must inspect the selected project folder, not a default session folder"
    );
    assert!(
        requests[0]["tools"]
            .as_array()
            .is_some_and(|t| t.iter().any(|v| v["function"]["name"] == "delegate_task")),
        "{}",
        requests[0]
    );
    assert_eq!(
        requests.len(),
        5,
        "delegation, fs_list, fs_read, child result, conductor result"
    );
    assert!(
        requests[3]["messages"]
            .to_string()
            .contains("selected-project-marker"),
        "Delta must read the content from the selected project's README"
    );
    let runs = c.agents_runs(sid).await.unwrap();
    assert!(
        runs.iter().any(|r| r.run.agent == "delta"
            && r.steps.iter().any(|s| s.tool.as_deref() == Some("fs_list")))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn queued_model_missing_from_static_map_actually_delegates_and_reads_directory() {
    run_case(false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn text_only_first_choice_uses_next_tool_capable_model_in_queue() {
    run_case(true).await;
}
