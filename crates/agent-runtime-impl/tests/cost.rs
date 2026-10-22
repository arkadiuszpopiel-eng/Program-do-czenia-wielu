//! Przegląd 2026-10-21: koszt tury liczony z modelu, który odpowiedział (`Started`; Router
//! podaje `dostawca:model`) — bez przypięcia `spec.model` to „auto” i koszt wychodził 0, więc
//! limit miesięczny i „Limit kosztu zadania” nie widziały wydatków agentek.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use agent_runtime_contract::{AgentRuntime, MemCheckpointStore, RunEvent, contract_tests};
use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps};
use providers_contract::{Pricing, ProviderEvent, StopReason, ToolArguments, Usage};
use providers_fake::{FakeProvider, Script, Step};
use tools_common_contract::{ToolOutcome, Toolset};
use tools_fs_contract::FsToolKind;
use tools_fs_fake::FakeFsTools;

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

/// Model przypięty, a odpowiedź z nazwą spoza cennika (np. z datą wydania): cena modelu
/// przypiętego — jak w zwykłej rozmowie (przegląd poprawki: wcześniej koszt znikał).
#[tokio::test(start_paused = true)]
async fn pinned_model_price_is_the_fallback() {
    let provider = FakeProvider::new("anthropic").with_pricing(
        "claude-x",
        Pricing {
            input_per_mtok_usd: 3.0,
            output_per_mtok_usd: 15.0,
            cache_read_per_mtok_usd: None,
            cache_write_per_mtok_usd: None,
        },
    );
    provider.push_script(Script::new(vec![
        Step::Emit(ProviderEvent::Started {
            model: "claude-x-20260101".into(),
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
    let mut spec = contract_tests::sample_spec("claude-x", &[]);
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
    assert_eq!(cost, 3_000_000_000);
}

fn priced(input_per_mtok_usd: f64) -> Pricing {
    Pricing {
        input_per_mtok_usd,
        output_per_mtok_usd: 0.0,
        cache_read_per_mtok_usd: None,
        cache_write_per_mtok_usd: None,
    }
}

/// Tura modelu `model` (100 tys. tokenów wejścia; budżet przebiegu to 200 tys.): odpowiedź `text` albo wywołanie `fs_list`.
fn turn(model: &str, text: Option<&str>) -> Script {
    let mut steps = vec![Step::Emit(ProviderEvent::Started {
        model: model.into(),
        response_id: None,
    })];
    match text {
        Some(text) => steps.push(Step::Emit(ProviderEvent::TextDelta {
            index: 0,
            text: text.into(),
        })),
        None => steps.extend([
            Step::Emit(ProviderEvent::ToolCallStart {
                index: 0,
                id: "t1".into(),
                name: "fs_list".into(),
            }),
            Step::Emit(ProviderEvent::ToolCallEnd {
                index: 0,
                id: "t1".into(),
                arguments: ToolArguments::Parsed {
                    value: serde_json::json!({"path": "/Users/ala/Documents"}),
                },
            }),
        ]),
    }
    steps.push(Step::Emit(ProviderEvent::Usage(Usage {
        input_tokens: 100_000,
        output_tokens: 0,
        ..Usage::default()
    })));
    let stop = if text.is_some() {
        StopReason::EndTurn
    } else {
        StopReason::ToolUse
    };
    steps.push(Step::Emit(ProviderEvent::stop(stop)));
    Script::new(steps)
}

/// Przegląd 2026-10-22: koszt przebiegu przez Router zapisywał się pod „router”, więc limit konta
/// (dostawcy) go nie widział. Każda tura podaje model, który odpowiedział, i swój koszt; suma części
/// równa się sumie przebiegu.
#[tokio::test(start_paused = true)]
async fn each_turn_reports_the_model_that_answered_and_its_cost() {
    let provider = FakeProvider::new("router")
        .with_pricing("anthropic:claude-x", priced(30.0))
        .with_pricing("openai:gpt-x", priced(10.0));
    provider.push_script(turn("anthropic:claude-x", None));
    provider.push_script(turn("openai:gpt-x", Some("Gotowe.")));
    let fs = FakeFsTools::new();
    fs.push(
        FsToolKind::List,
        ToolOutcome::ok("faktura.pdf", serde_json::json!({})),
    );
    let runtime = Runtime::new(RuntimeDeps {
        provider: Arc::new(provider.clone()),
        tools: fs.tools(),
        checkpoints: Arc::new(MemCheckpointStore::default()),
        bus: None,
        config: RuntimeConfig::default(),
    });
    let mut spec = contract_tests::sample_spec("auto", &["fs_list"]);
    spec.verify = false;
    let run = runtime.start(spec).await.unwrap();
    runtime.wait(&run).await.unwrap();
    let events = runtime.events(&run).unwrap();
    let parts: Vec<_> = events
        .iter()
        .filter_map(|e| match &e.event {
            RunEvent::ModelUsage {
                model,
                input_tokens,
                cost_nano_usd,
                ..
            } => Some((model.clone(), *input_tokens, cost_nano_usd.unwrap_or(0))),
            _ => None,
        })
        .collect();
    assert_eq!(
        parts,
        vec![
            (Some("anthropic:claude-x".into()), 100_000, 3_000_000_000),
            (Some("openai:gpt-x".into()), 100_000, 1_000_000_000),
        ]
    );
    let total = events
        .iter()
        .filter_map(|e| match &e.event {
            RunEvent::Usage(u) => Some(u.cost_nano_usd),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    assert_eq!(total, parts.iter().map(|p| p.2).sum::<u64>());
}
