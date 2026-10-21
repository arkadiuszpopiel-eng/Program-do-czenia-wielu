//! Przegląd 2026-10-21: koszt tury liczony z modelu, który odpowiedział (`Started`; Router
//! podaje `dostawca:model`) — bez przypięcia `spec.model` to „auto” i koszt wychodził 0, więc
//! limit miesięczny i „Limit kosztu zadania” nie widziały wydatków agentek.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use agent_runtime_contract::{AgentRuntime, MemCheckpointStore, RunEvent, contract_tests};
use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps};
use providers_contract::{Pricing, ProviderEvent, StopReason, Usage};
use providers_fake::{FakeProvider, Script, Step};

#[tokio::test(start_paused = true)]
async fn turn_cost_uses_the_model_that_answered() {
    let provider = FakeProvider::new("router").with_pricing(
        "anthropic:claude-x",
        Pricing {
            input_per_mtok_usd: 3.0,
            output_per_mtok_usd: 15.0,
            cache_read_per_mtok_usd: None,
            cache_write_per_mtok_usd: None,
        },
    );
    provider.push_script(Script::new(vec![
        Step::Emit(ProviderEvent::Started {
            model: "anthropic:claude-x".into(),
            response_id: None,
        }),
        Step::Emit(ProviderEvent::TextDelta {
            index: 0,
            text: "Gotowe.".into(),
        }),
        Step::Emit(ProviderEvent::Usage(Usage {
            input_tokens: 1_000_000,
            output_tokens: 0,
            ..Usage::default()
        })),
        Step::Emit(ProviderEvent::stop(StopReason::EndTurn)),
    ]));
    let runtime = Runtime::new(RuntimeDeps {
        provider: Arc::new(provider.clone()),
        tools: Vec::new(),
        checkpoints: Arc::new(MemCheckpointStore::default()),
        bus: None,
        config: RuntimeConfig::default(),
    });
    let mut spec = contract_tests::sample_spec("auto", &[]);
    spec.verify = false;
    let run = runtime.start(spec).await.unwrap();
    runtime.wait(&run).await.unwrap();
    let cost = runtime
        .events(&run)
        .unwrap()
        .iter()
        .filter_map(|e| match &e.event {
            RunEvent::Usage(u) => Some(u.cost_nano_usd),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    // 1 mln tokenów wejścia po 3 USD = 3 USD.
    assert_eq!(cost, 3_000_000_000);
}
