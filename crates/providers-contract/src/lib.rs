//! Wspólny kontrakt `ModelProvider` (docs/PLAN.md §5.1–5.3, ADR 0005, ADR 0006).
//!
//! Zawiera wyłącznie typy, trait i czyste funkcje — bez sieci:
//! - **neutralny IR rozmowy** ([`Message`], [`ContentBlock`], [`ChatRequest`]) — bloki myślenia
//!   z nieprzezroczystym podpisem i [`ProviderOrigin`], narzędzia z JSON Schema i `strict`,
//!   parametry (`max_tokens`, `temperature`, [`Effort`], `stop`), tag prywatności żądania;
//! - **zdarzenia strumienia** ([`ProviderEvent`]) i ich złożenie w turę ([`TurnAccumulator`]);
//! - **błędy** sklasyfikowane dla Routera ([`ProviderError`], [`ProviderErrorKind`]);
//! - **możliwości** ([`ModelCapabilities`]), **koszt** z tabeli cen ([`Pricing`]),
//!   **prywatność** ([`check_privacy`]), **przerwane tury** ([`render_interrupted_turn`]);
//! - pod feature `contract-tests` — [`contract_tests`] uruchamiane na atrapie i na każdym adapterze.
//!
//! Implementacje: `providers-api-impl` (Anthropic, OpenAI, adapter generyczny), `providers-local`
//! (llama.cpp), `providers-fake` (atrapa record/replay). Router i inne moduły zależą **tylko** od
//! tego crate'a.
//!
//! ```
//! use providers_contract::*;
//! let req = ChatRequest::new("claude-opus-5-5", vec![Message::user_text("Która godzina?")])
//!     .with_system("Jesteś Alfą.")
//!     .with_tool(ToolSpec {
//!         name: "clock".into(),
//!         description: "Podaje godzinę".into(),
//!         input_schema: serde_json::json!({"type": "object", "properties": {},
//!                                          "additionalProperties": false, "required": []}),
//!         strict: true,
//!     });
//! assert!(req.validate().is_ok());
//! ```

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod accumulate;
mod capabilities;
mod error;
mod event;
mod interruption;
mod message;
mod pricing;
mod privacy;
mod provider;
mod request;
mod schema;
mod secret;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use accumulate::{AssistantTurn, TurnAccumulator};
pub use capabilities::{
    ModelCapabilities, ModelInfo, ModelKind, ProviderCapabilities, ThinkingSupport,
};
pub use error::{
    ProviderError, ProviderErrorKind, TimeoutPhase, classify_http_status, parse_retry_after_ms,
};
pub use event::{ProviderEvent, StopDetails, StopReason, ToolArguments, Usage};
pub use interruption::{
    InterruptionRendering, interruption_note, project_history, render_interrupted_turn,
};
pub use message::{
    ContentBlock, ImageSource, Interruption, Message, ProviderId, ProviderOrigin,
    RedactedThinkingBlock, Role, ThinkingBlock, ToolResult, ToolResultPart, ToolUse,
};
pub use pricing::{
    Cost, CostEstimate, IMAGE_TOKEN_ESTIMATE, Pricing, PricingTable, estimate_input_tokens,
};
pub use privacy::{PrivacyTag, ProviderPrivacy, RequestPrivacy, check_privacy};
pub use provider::{
    EmbeddingRequest, EmbeddingResponse, HealthState, ModelProvider, ProviderHealth,
    ProviderStream, events,
};
pub use request::{
    CachePolicy, CacheTtl, ChatRequest, Effort, GenerationParams, RequestMeta, ThinkingConfig,
    ThinkingDisplay, ToolChoice, ToolSpec, valid_tool_name,
};
pub use schema::{IR_SCHEMA_VERSION, chat_request_schema, provider_event_schema};
pub use secret::{ApiKey, REDACTED_KEY, SecretSource, StaticKey};
/// Token anulowania używany w [`ModelProvider::stream`] (re-eksport `tokio-util`).
pub use tokio_util::sync::CancellationToken;
