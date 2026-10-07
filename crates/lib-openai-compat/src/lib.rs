//! Wspólna biblioteka klientów HTTP „zgodnych z OpenAI" (bez logiki modułu; crates/README.md §`lib-*`).
//!
//! Wydzielona z `providers-api-impl`, żeby `providers-local-impl` (sidecar `llama-server`)
//! mógł używać tego samego, przetestowanego silnika bez zależności od cudzego `-impl`:
//! - [`Engine`] + [`WireCodec`]/[`StreamDecoder`] — strumień SSE w zadaniu tła, ponawianie
//!   z backoffem i jitterem **tylko przed pierwszym tokenem**, limity connect/first-token/idle,
//!   anulowanie zrywające połączenie ≤ 100 ms, klucz z `SecretSource` w chwili wywołania
//!   (nagłówek wrażliwy, redakcja błędów), zdrowie z historii wywołań;
//! - [`sse`] — parser Server-Sent Events odporny na podział porcji w dowolnym bajcie;
//! - [`chat`] — format Chat Completions (`build_body` + [`chat::ChatDecoder`]);
//! - [`common`] — klasyfikacja błędów OpenAI, zużycie, obrazy, wysiłek;
//! - konfiguracja: [`HttpConfig`], [`AuthScheme`], [`Timeouts`], [`RetryPolicy`], [`ProviderProfile`].
//!
//! Zależy wyłącznie od `providers-contract` (i crate'ów zewnętrznych).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod chat;
pub mod common;
mod config;
mod engine;
mod retry;
mod run;
pub mod sse;

pub use chat::{ChatDecoder, ChatOptions, MaxTokensField};
pub use config::{AuthScheme, ConfigError, HttpConfig, ProviderProfile, RetryPolicy, Timeouts};
pub use engine::{BuildOptions, Engine, StreamDecoder, WireCodec, WireRequest, cancelled, single};
