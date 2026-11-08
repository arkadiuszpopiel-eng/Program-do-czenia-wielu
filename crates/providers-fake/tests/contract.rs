//! Zestaw kontraktowy `ModelProvider` na atrapie (czas wirtualny tokio).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use providers_contract::contract_tests::{self, Harness, Scenario};
use providers_contract::{ModelProvider, Pricing, ProviderPrivacy};
use providers_fake::{FAKE_MODEL, FakeProvider, FakeTimeouts, Script};

const STALL: Duration = Duration::from_millis(300);

fn pricing() -> Pricing {
    Pricing {
        input_per_mtok_usd: 1.0,
        output_per_mtok_usd: 2.0,
        cache_read_per_mtok_usd: Some(0.1),
        cache_write_per_mtok_usd: None,
    }
}

#[derive(Default)]
struct FakeHarness {
    last: Mutex<Option<FakeProvider>>,
}

impl FakeHarness {
    fn remember(&self, fake: FakeProvider) -> FakeProvider {
        *self.last.lock().unwrap() = Some(fake.clone());
        fake
    }

    fn base() -> FakeProvider {
        FakeProvider::new("fake")
            .with_pricing(FAKE_MODEL, pricing())
            .with_timeouts(FakeTimeouts {
                first_token: Some(STALL),
                idle: Some(STALL),
            })
    }
}

#[async_trait]
impl Harness for FakeHarness {
    type Provider = FakeProvider;

    async fn provider(&self, scenario: Scenario) -> Option<FakeProvider> {
        let script = match scenario {
            Scenario::Text { chunks } => Script::chunks(FAKE_MODEL, &chunks, Duration::ZERO),
            Scenario::ToolCall {
                id,
                name,
                arguments,
            } => Script::tool_call(FAKE_MODEL, &id, &name, &arguments),
            Scenario::Thinking {
                thinking,
                signature,
                text,
            } => Script::thinking(FAKE_MODEL, &thinking, &signature, &text),
            Scenario::HttpError {
                status,
                retry_after_s,
            } => Script::http_error(status, retry_after_s),
            Scenario::Refusal => Script::refusal(FAKE_MODEL),
            Scenario::MaxTokens { text } => Script::max_tokens(FAKE_MODEL, &text),
            Scenario::Stall => Script::stall(),
            Scenario::Slow { chunks, interval } => Script::chunks(FAKE_MODEL, &chunks, interval),
        };
        Some(self.remember(Self::base().with_default_script(script)))
    }

    async fn provider_blocking_private(&self) -> FakeProvider {
        self.remember(
            Self::base()
                .with_privacy(ProviderPrivacy::new("cn-may-train", "CN"))
                .with_default_script(Script::text(FAKE_MODEL, &["x"])),
        )
    }

    fn wire_requests(&self) -> usize {
        self.last
            .lock()
            .unwrap()
            .as_ref()
            .map_or(0, |f| f.requests().len())
    }

    fn last_wire_request(&self) -> Option<String> {
        let last = self.last.lock().unwrap();
        let req = last.as_ref()?.requests().pop()?;
        serde_json::to_string(&req).ok()
    }

    fn model(&self) -> String {
        FAKE_MODEL.into()
    }

    fn pricing(&self) -> Pricing {
        pricing()
    }

    fn stall_timeout(&self) -> Duration {
        STALL
    }
}

#[tokio::test(start_paused = true)]
async fn contract_suite_on_fake() {
    contract_tests::run_all(&FakeHarness::default()).await;
}

#[tokio::test]
async fn contract_suite_on_fake_real_time() {
    // Ten sam zestaw w czasie rzeczywistym: anulowanie ≤ 100 ms i timeouty mierzone zegarem ściennym.
    contract_tests::run_all(&FakeHarness::default()).await;
}

#[tokio::test]
async fn arc_provider_is_a_provider() {
    let fake = std::sync::Arc::new(FakeProvider::new("arc"));
    let dynamic: std::sync::Arc<dyn ModelProvider> = fake;
    assert_eq!(dynamic.id().as_str(), "arc");
    assert_eq!(dynamic.list_models().await.unwrap().len(), 1);
}
