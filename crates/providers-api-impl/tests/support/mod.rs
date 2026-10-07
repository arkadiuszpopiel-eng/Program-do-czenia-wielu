//! Wspólne narzędzia testów adapterów: serwer fixture, nagrania SSE, uchwyt testu kontraktowego.

#![allow(dead_code)]

pub mod anthropic_sse;
pub mod openai_sse;
pub mod server;

use std::time::Duration;

use async_trait::async_trait;
use providers_api_impl::{AuthScheme, HttpConfig, ProviderProfile, RetryPolicy, Timeouts};
use providers_contract::contract_tests::{Harness, Scenario};
use providers_contract::{ModelCapabilities, ModelProvider, Pricing, ProviderPrivacy};
pub use server::{FixtureServer, Part, Reply};

/// Limit „first-token"/idle w testach (scenariusz `Stall`).
pub const STALL: Duration = Duration::from_millis(300);

pub fn pricing() -> Pricing {
    Pricing {
        input_per_mtok_usd: 4.0,
        output_per_mtok_usd: 20.0,
        cache_read_per_mtok_usd: Some(0.2),
        cache_write_per_mtok_usd: Some(5.0),
    }
}

/// Profil testowy (`eu`/`EU`, cennik dla `model`).
pub fn profile(id: &str, model: &str) -> ProviderProfile {
    let mut p = ProviderProfile::new(id);
    p.privacy = ProviderPrivacy::new("eu", "EU");
    p.default_model = Some(model.to_owned());
    p.pricing.insert(model.to_owned(), pricing());
    p
}

/// Szybkie limity i ponawianie do testów (≤ 2 s na fallback).
pub fn http(server: &FixtureServer, auth: AuthScheme) -> HttpConfig {
    let mut h = HttpConfig::new(server.url(), auth);
    h.timeouts = Timeouts {
        connect: Duration::from_secs(1),
        first_token: STALL,
        idle: STALL,
    };
    h.retry = RetryPolicy {
        max_retries: 1,
        base_delay: Duration::from_millis(20),
        max_delay: Duration::from_millis(50),
        max_retry_after: Duration::from_millis(500),
        budget: Duration::from_millis(800),
    };
    h
}

/// Wolny strumień: `prefix`, potem fragmenty z pauzami, potem `suffix`.
pub fn slow(prefix: String, chunks: Vec<String>, interval: Duration, suffix: String) -> Reply {
    let mut parts = vec![Part::Bytes(prefix.into_bytes())];
    for c in chunks {
        parts.push(Part::Bytes(c.into_bytes()));
        parts.push(Part::Sleep(interval));
    }
    parts.push(Part::Bytes(suffix.into_bytes()));
    Reply::sse_parts(parts)
}

/// Odpowiedź błędu HTTP z opcjonalnym `retry-after`.
pub fn http_error(status: u16, retry_after_s: Option<u64>, body: &serde_json::Value) -> Reply {
    let r = Reply::json(status, body);
    match retry_after_s {
        Some(s) => r.with_header("retry-after", &s.to_string()),
        None => r,
    }
}

type ReplyFn = Box<dyn Fn(&Scenario) -> Option<Reply> + Send + Sync>;
type MakeFn<P> = Box<dyn Fn(&FixtureServer, ProviderProfile) -> P + Send + Sync>;

/// Uchwyt testu kontraktowego dla adaptera HTTP z serwerem fixture.
pub struct WireHarness<P> {
    pub server: FixtureServer,
    pub id: String,
    pub model: String,
    pub caps: Option<ModelCapabilities>,
    pub reply: ReplyFn,
    pub make: MakeFn<P>,
}

impl<P> WireHarness<P> {
    pub async fn new(id: &str, model: &str, reply: ReplyFn, make: MakeFn<P>) -> Self {
        Self {
            server: FixtureServer::start().await,
            id: id.into(),
            model: model.into(),
            caps: None,
            reply,
            make,
        }
    }

    fn profile(&self) -> ProviderProfile {
        let mut p = profile(&self.id, &self.model);
        if let Some(c) = &self.caps {
            p.models.insert(self.model.clone(), c.clone());
        }
        p
    }
}

#[async_trait]
impl<P: ModelProvider + 'static> Harness for WireHarness<P> {
    type Provider = P;

    async fn provider(&self, scenario: Scenario) -> Option<P> {
        let reply = (self.reply)(&scenario)?;
        self.server.reset(reply);
        Some((self.make)(&self.server, self.profile()))
    }

    async fn provider_blocking_private(&self) -> P {
        self.server.reset(Reply::sse(String::new()));
        let mut p = self.profile();
        p.privacy = ProviderPrivacy::new("cn-may-train", "CN");
        (self.make)(&self.server, p)
    }

    fn wire_requests(&self) -> usize {
        self.server.requests().len()
    }

    fn last_wire_request(&self) -> Option<String> {
        self.server.requests().pop().map(|r| r.body)
    }

    fn model(&self) -> String {
        self.model.clone()
    }

    fn pricing(&self) -> Pricing {
        pricing()
    }

    fn stall_timeout(&self) -> Duration {
        STALL
    }
}
