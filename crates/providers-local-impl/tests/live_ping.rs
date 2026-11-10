//! Ręczne odtworzenie żądania na lokalnym modelu: zimny i ciepły adapter, bez Core/GPU/mikrofonu.
//! ALFA_PING_SERVER, ALFA_PING_MODELS, ALFA_PING_INPUT, ALFA_PING_REPORT; wejście: {system,query}.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use futures_util::StreamExt;
use providers_contract::{CancellationToken, ChatRequest, Message, ModelProvider, ProviderEvent};
use providers_local_impl::{
    LaunchSpec, LocalConfig, LocalError, LocalProvider, Sidecar, SidecarLauncher, SidecarProcess,
    TokioLauncher, builtin_models,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct ObserveLaunch(Mutex<Vec<String>>);

impl SidecarLauncher for ObserveLaunch {
    fn spawn(&self, spec: &LaunchSpec) -> Result<Box<dyn SidecarProcess>, LocalError> {
        // Odczyt wyłącznie jawnej kadencji; nie zapisujemy klucza ani całego argv.
        let ping = spec
            .args
            .windows(2)
            .find(|a| a[0] == "--sse-ping-interval")
            .map_or("default", |a| a[1].as_str());
        self.0.lock().unwrap().push(ping.into());
        TokioLauncher::default().spawn(spec)
    }
}

async fn sample(provider: &LocalProvider, request: ChatRequest, phase: &str) -> Value {
    let start = Instant::now();
    let mut first_visible = None;
    let mut text = String::new();
    let mut received = Vec::new();
    let mut stream = provider.stream(request, CancellationToken::new());
    let finished = tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(event) = stream.next().await {
            if let ProviderEvent::TextDelta { text: chunk, .. } = &event
                && !chunk.is_empty()
            {
                first_visible.get_or_insert_with(|| start.elapsed().as_millis());
                text.push_str(chunk);
            }
            received.push(event);
        }
    })
    .await
    .is_ok();
    let errors: Vec<_> = received
        .iter()
        .filter(|e| matches!(e, ProviderEvent::Error(_)))
        .collect();
    let stopped = matches!(received.last(), Some(ProviderEvent::Stop { .. }));
    json!({"phase":phase,"elapsed_ms":start.elapsed().as_millis(),"text":text,
        "first_visible_ms":first_visible,"first_visible_within60s":first_visible.is_some_and(|t|t<=60_000),
        "completed_within120s":finished,"stopped":stopped,"errors":errors,
        "events":received})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "rzeczywisty model CPU; ALFA_PING_*; surowy raport i TTFT oddzielnie od pingów"]
async fn cold_and_warm_request_keep_transport_and_visible_budgets_separate() {
    let input: Value =
        serde_json::from_slice(&std::fs::read(std::env::var("ALFA_PING_INPUT").unwrap()).unwrap())
            .unwrap();
    let mut config = LocalConfig::new(
        std::env::var("ALFA_PING_MODELS").unwrap(),
        std::env::var("ALFA_PING_SERVER").unwrap(),
    );
    config.threads = Some(4);
    let model = config.default_model.clone();
    let launcher = Arc::new(ObserveLaunch::default());
    let sidecar = Sidecar::new(config, launcher.clone(), None, None).unwrap();
    let provider = LocalProvider::new(builtin_models().unwrap(), sidecar.clone());
    let mut request = ChatRequest::new(
        &model,
        vec![Message::user_text(input["query"].as_str().unwrap())],
    );
    request.system = Some(input["system"].as_str().unwrap().into());
    request.params.max_tokens = Some(96);
    request.params.temperature = Some(0.0);
    let cold = sample(&provider, request.clone(), "cold").await;
    let results = if cold["errors"].as_array().unwrap().is_empty() && cold["stopped"] == true {
        vec![cold, sample(&provider, request, "warm").await]
    } else {
        vec![cold]
    };
    sidecar.stop("koniec ręcznej próby pingów").await;
    let report = json!({"scope":"real LocalProvider replay; no Core, GPU, microphone or physical action",
        "model":model,"threads":4,"context":8192,"max_tokens":96,"temperature":0,
        "first_event_s":60,"idle_s":30,"turn_s":120,"visible_budget_ms":60000,
        "transport_success_is_not_timing_acceptance":true,
        "launch_ping_seconds":*launcher.0.lock().unwrap(),"input":input,"results":results});
    std::fs::write(
        std::env::var("ALFA_PING_REPORT").unwrap(),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert_eq!(results.len(), 2, "niewykonane próby nie są zaliczone");
    for result in results {
        assert_eq!(result["completed_within120s"], true);
        assert_eq!(result["stopped"], true);
        assert_eq!(result["errors"], json!([]));
        assert_eq!(result["first_visible_within60s"], true);
    }
}
