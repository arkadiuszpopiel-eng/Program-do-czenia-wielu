//! Przegląd 2026-10-22: błąd dostawcy przerywający przebieg niesie model, który zawiódł (`Started`
//! przed błędem — Router podaje w nim `dostawca:model`), żeby rozmowa z agentką nazwała dostawcę
//! zamiast „Router”.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use agent_runtime_contract::{AgentRuntime, MemCheckpointStore, RunOutcome, contract_tests};
use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps};
use providers_contract::{ProviderError, ProviderErrorKind, ProviderEvent};
use providers_fake::{FakeProvider, Script, Step};

#[tokio::test(start_paused = true)]
async fn provider_failure_names_the_model_that_failed() {
    let provider = FakeProvider::new("router");
    provider.push_script(Script::new(vec![
        Step::Emit(ProviderEvent::Started {
            model: "openai:gpt-x".into(),
            response_id: None,
        }),
        Step::Emit(ProviderEvent::Error(ProviderError::new(
            ProviderErrorKind::RateLimited {
                retry_after_ms: Some(30_000),
            },
            "rate limit",
        ))),
    ]));
    let runtime = Runtime::new(RuntimeDeps {
        provider: Arc::new(provider),
        tools: Vec::new(),
        checkpoints: Arc::new(MemCheckpointStore::default()),
        bus: None,
        config: RuntimeConfig::default(),
    });
    let mut spec = contract_tests::sample_spec("auto", &[]);
    spec.verify = false;
    let run = runtime.start(spec).await.unwrap();
    let outcome = runtime.wait(&run).await.unwrap();
    let RunOutcome::Failed {
        provider, model, ..
    } = outcome
    else {
        panic!("oczekiwany błąd przebiegu: {outcome:?}");
    };
    assert!(matches!(
        provider.map(|e| e.kind),
        Some(ProviderErrorKind::RateLimited { .. })
    ));
    assert_eq!(model.as_deref(), Some("openai:gpt-x"));
}
