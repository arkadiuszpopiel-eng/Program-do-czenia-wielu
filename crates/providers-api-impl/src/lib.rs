//! Adaptery `ModelProvider` dla API chmurowych (docs/modules/providers-api/SPEC.md, PLAN §5.2–5.6).
//!
//! - [`AnthropicProvider`] — Messages API (SSE, narzędzia z `strict`, myślenie odsyłane bez zmian,
//!   cache promptu `tools → system → messages`, jawny `effort`, `refusal`, serwerowy fallback,
//!   429 `retry-after`, 529 overloaded) oraz „endpoint zgodny z Anthropic";
//! - [`OpenAiProvider`] — Chat Completions i Responses API oraz „endpoint zgodny z OpenAI"
//!   (xAI, DeepSeek, Kimi, Qwen, Z.ai, MiniMax, OpenRouter, Mistral, Ollama, LM Studio…);
//! - [`build_provider`] — adapter generyczny z wpisu `providers-catalog/<id>.toml` + konta;
//! - wspólny silnik: ponawianie z backoffem i jitterem **tylko przed pierwszym tokenem**
//!   i tylko dla odrzuceń idempotentnych, limity czasu connect/first-token/idle, anulowanie
//!   zrywające połączenie ≤ 100 ms, koszt z tabeli cen konfiguracji, Models API;
//! - [`ProvidersApiModule`] (`module.toml`) i [`ObservedProvider`] (zdarzenia `provider.*`).
//!
//! Klucze: wyłącznie przez `SecretSource` w chwili wywołania; nagłówki oznaczone jako wrażliwe,
//! komunikaty błędów redagowane. Brak OpenSSL (rustls + ring, certyfikaty systemu).
//!
//! ```no_run
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! use std::sync::Arc;
//! use providers_api_impl::{AccountProfile, CatalogEntry, build_provider};
//! use providers_contract::StaticKey;
//!
//! let entry = CatalogEntry::parse_toml(include_str!("../../../providers-catalog/xai.toml"))?;
//! let mut account = AccountProfile::new(Arc::new(StaticKey::new("xai-…")));
//! account.default_model = Some("grok-5".into());
//! let provider = build_provider(&entry, account)?; // Arc<dyn ModelProvider>
//! let models = provider.list_models().await?;
//! # let _ = models; Ok(()) }
//! ```

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod anthropic;
mod catalog;
mod config;
mod engine;
mod module;
mod observe;
pub mod openai;
mod registry;
mod retry;
mod run;
pub mod sse;

pub use anthropic::{AnthropicOptions, AnthropicProvider, PrefixMismatch};
pub use catalog::{
    AccountProfile, CatalogAuth, CatalogCapabilities, CatalogCompat, CatalogEntry, CatalogKind,
    Tribool, build_provider,
};
pub use config::{AuthScheme, ConfigError, HttpConfig, ProviderProfile, RetryPolicy, Timeouts};
pub use module::{MODULE_TOML, ProvidersApiModule};
pub use observe::ObservedProvider;
pub use openai::{MaxTokensField, OpenAiApi, OpenAiOptions, OpenAiProvider};
