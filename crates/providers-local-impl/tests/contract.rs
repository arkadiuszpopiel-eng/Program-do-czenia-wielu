//! Zestaw kontraktowy `ModelProvider` na dostawcy lokalnym z prawdziwym procesem
//! (fałszywy `llama-server`): start na żądanie, strumień, błędy HTTP, timeout, anulowanie ≤ 100 ms.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod support;

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use providers_contract::contract_tests::{self, Harness, Scenario};
use providers_contract::{Pricing, ProviderPrivacy};
use providers_local_impl::LocalProvider;
use serde_json::json;
use support::{Env, MODEL};

#[derive(Default)]
struct LocalHarness {
    last: Mutex<Option<Env>>,
}

impl LocalHarness {
    fn make(
        &self,
        scenario: Option<serde_json::Value>,
        privacy: Option<ProviderPrivacy>,
    ) -> LocalProvider {
        let env = Env::new();
        let text = scenario.map(|s| s.to_string());
        let mut config = env.config();
        if let Some(p) = privacy {
            config.privacy = p;
        }
        let provider = env.provider_with(config, env.launcher("ok", text.as_deref()), None, None);
        *self.last.lock().unwrap() = Some(env);
        provider
    }
}

#[async_trait]
impl Harness for LocalHarness {
    type Provider = LocalProvider;

    async fn provider(&self, scenario: Scenario) -> Option<LocalProvider> {
        let s = match scenario {
            Scenario::Text { chunks } => json!({"kind": "text", "chunks": chunks}),
            Scenario::ToolCall {
                id,
                name,
                arguments,
            } => {
                json!({"kind": "tool", "id": id, "name": name, "arguments": arguments})
            }
            // Chat Completions nie przenosi podpisanego myślenia.
            Scenario::Thinking { .. } => return None,
            Scenario::HttpError {
                status,
                retry_after_s,
            } => {
                json!({"kind": "http_error", "status": status, "retry_after": retry_after_s})
            }
            Scenario::Refusal => json!({"kind": "refusal"}),
            Scenario::MaxTokens { text } => json!({"kind": "max_tokens", "text": text}),
            Scenario::Stall => json!({"kind": "stall"}),
            Scenario::Slow { chunks, interval } => json!({"kind": "slow", "chunks": chunks,
                "interval_ms": u64::try_from(interval.as_millis()).unwrap()}),
        };
        Some(self.make(Some(s), None))
    }

    async fn provider_blocking_private(&self) -> LocalProvider {
        self.make(None, Some(ProviderPrivacy::new("cn-may-train", "CN")))
    }

    fn wire_requests(&self) -> usize {
        self.last
            .lock()
            .unwrap()
            .as_ref()
            .map_or(0, |e| e.requests().len())
    }

    fn last_wire_request(&self) -> Option<String> {
        self.last.lock().unwrap().as_ref()?.requests().pop()
    }

    fn model(&self) -> String {
        MODEL.into()
    }

    fn pricing(&self) -> Pricing {
        Pricing {
            input_per_mtok_usd: 0.0,
            output_per_mtok_usd: 0.0,
            cache_read_per_mtok_usd: None,
            cache_write_per_mtok_usd: None,
        }
    }

    fn stall_timeout(&self) -> Duration {
        Duration::from_millis(600)
    }
}

/// Pełny zestaw ze ścisłymi budżetami (2 s / 100 ms) przy `ALFA_PERF_BUDGETS=1`; na współdzielonym
/// CI — przypadki bez budżetów czasu + wersje czasowe z progiem ×10 (crates/README.md).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn contract_suite_on_local_sidecar() {
    let h = LocalHarness::default();
    if std::env::var_os("ALFA_PERF_BUDGETS").is_some() {
        contract_tests::run_all(&h).await;
        return;
    }
    contract_tests::text_stream_follows_grammar(&h).await;
    contract_tests::tool_call_is_assembled(&h).await;
    contract_tests::thinking_is_signed_and_replayed(&h).await;
    contract_tests::refusal_is_a_stop_not_an_error(&h).await;
    contract_tests::max_tokens_is_reported(&h).await;
    contract_tests::pre_cancelled_sends_nothing(&h).await;
    contract_tests::private_request_is_blocked_before_wire(&h).await;
    contract_tests::invalid_request_is_rejected_locally(&h).await;
    contract_tests::interrupted_turn_is_rendered_append_only(&h).await;
    contract_tests::cost_comes_from_pricing(&h).await;
}

/// Klasyfikacja błędów HTTP i milczenia serwera (z progiem czasu ×10 poza maszyną pomiarową).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn errors_and_stall_are_classified_with_relaxed_budget() {
    use providers_contract::{
        CancellationToken, ChatRequest, Message, ModelProvider, ProviderErrorKind, ProviderEvent,
    };
    let h = LocalHarness::default();
    let cases: [(u16, Option<u64>, ProviderErrorKind); 4] = [
        (400, None, ProviderErrorKind::InvalidRequest),
        (401, None, ProviderErrorKind::Auth),
        (
            429,
            Some(30),
            ProviderErrorKind::RateLimited {
                retry_after_ms: Some(30_000),
            },
        ),
        (500, None, ProviderErrorKind::Server { status: 500 }),
    ];
    for (status, retry_after_s, expected) in cases {
        let p = h
            .provider(Scenario::HttpError {
                status,
                retry_after_s,
            })
            .await
            .unwrap();
        let req = ChatRequest::new(MODEL, vec![Message::user_text("x")]);
        let got = contract_tests::collect(p.stream(req, CancellationToken::new())).await;
        eprintln!(
            "HTTP {status}: błąd po {:?} (budżet 2 s z uruchomieniem sidecara)",
            got.terminal_at()
        );
        assert!(
            matches!(got.terminal(), ProviderEvent::Error(e) if e.kind == expected),
            "{:?}",
            got.events
        );
        assert!(got.terminal_at() <= support::budget(Duration::from_secs(2)));
    }
    let p = h.provider(Scenario::Stall).await.unwrap();
    let req = ChatRequest::new(MODEL, vec![Message::user_text("x")]);
    let got = contract_tests::collect(p.stream(req, CancellationToken::new())).await;
    eprintln!("milczenie serwera: timeout po {:?}", got.terminal_at());
    assert!(
        matches!(got.terminal(), ProviderEvent::Error(e) if matches!(e.kind, ProviderErrorKind::Timeout { .. }))
    );
    assert!(got.terminal_at() <= support::budget(Duration::from_millis(1_200)));
}
