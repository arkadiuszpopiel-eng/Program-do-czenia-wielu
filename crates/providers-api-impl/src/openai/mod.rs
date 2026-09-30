//! Adapter OpenAI (Chat Completions i Responses API) oraz generyczny „endpoint zgodny z OpenAI".

mod chat;
mod common;
mod responses;

use std::sync::Arc;

use async_trait::async_trait;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, EmbeddingRequest, EmbeddingResponse,
    ModelCapabilities, ModelInfo, ModelKind, ModelProvider, ProviderCapabilities, ProviderError,
    ProviderErrorKind, ProviderEvent, ProviderHealth, ProviderId, ProviderStream, SecretSource,
    Usage,
};
use reqwest::header::HeaderMap;
use serde_json::{Value, json};

use crate::config::{AuthScheme, ConfigError, HttpConfig, ProviderProfile};
use crate::engine::{BuildOptions, Engine, StreamDecoder, WireCodec, WireRequest};
use crate::registry::{ModelRegistry, cost};
use crate::sse::SseEvent;

/// Oficjalny endpoint OpenAI (z `/v1`; profil może nadpisać).
pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// Format API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenAiApi {
    /// `POST /chat/completions` — wspólny mianownik endpointów zgodnych.
    ChatCompletions,
    /// `POST /responses` — natywne OpenAI z zaszyfrowanym rozumowaniem.
    Responses,
}

/// Nazwa pola limitu wyjścia w Chat Completions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxTokensField {
    /// `max_completion_tokens` (OpenAI, modele rozumujące).
    MaxCompletionTokens,
    /// `max_tokens` (większość endpointów zgodnych).
    MaxTokens,
}

impl MaxTokensField {
    /// Nazwa pola.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MaxCompletionTokens => "max_completion_tokens",
            Self::MaxTokens => "max_tokens",
        }
    }
}

/// Opcje adaptera OpenAI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiOptions {
    /// Oficjalne API (ostrożne domyślne możliwości: narzędzia tak) vs endpoint zgodny.
    pub native: bool,
    /// Format API.
    pub api: OpenAiApi,
    /// Pole limitu wyjścia (Chat Completions).
    pub max_tokens_field: MaxTokensField,
    /// `stream_options.include_usage` (niektóre serwery zgodne go nie przyjmują).
    pub stream_usage: bool,
}

impl OpenAiOptions {
    /// Oficjalne OpenAI przez Responses API.
    pub fn native() -> Self {
        Self {
            native: true,
            api: OpenAiApi::Responses,
            max_tokens_field: MaxTokensField::MaxCompletionTokens,
            stream_usage: true,
        }
    }

    /// Oficjalne OpenAI przez Chat Completions.
    pub fn native_chat() -> Self {
        Self {
            api: OpenAiApi::ChatCompletions,
            ..Self::native()
        }
    }

    /// Endpoint zgodny z OpenAI (Chat Completions, `max_tokens`).
    pub fn compatible() -> Self {
        Self {
            native: false,
            api: OpenAiApi::ChatCompletions,
            max_tokens_field: MaxTokensField::MaxTokens,
            stream_usage: true,
        }
    }
}

pub(crate) enum OpenAiDecoder {
    Chat(chat::ChatDecoder),
    Responses(responses::ResponsesDecoder),
}

impl StreamDecoder for OpenAiDecoder {
    fn on_event(&mut self, event: SseEvent) -> Vec<ProviderEvent> {
        match self {
            Self::Chat(d) => d.on_event(event),
            Self::Responses(d) => d.on_event(event),
        }
    }

    fn on_eof(&mut self) -> Vec<ProviderEvent> {
        match self {
            Self::Chat(d) => d.on_eof(),
            Self::Responses(d) => d.on_eof(),
        }
    }
}

pub(crate) struct OpenAiCodec {
    options: OpenAiOptions,
    profile: ProviderProfile,
    registry: Arc<ModelRegistry>,
}

impl WireCodec for OpenAiCodec {
    type Decoder = OpenAiDecoder;

    fn build(&self, req: &ChatRequest, _opts: BuildOptions) -> Result<WireRequest, ProviderError> {
        let caps = self.registry.caps(&req.model);
        let (path, body) = match self.options.api {
            OpenAiApi::ChatCompletions => (
                "/chat/completions",
                chat::build_body(req, &caps, &self.profile, &self.options)?,
            ),
            OpenAiApi::Responses => (
                "/responses",
                responses::build_body(req, &caps, &self.profile)?,
            ),
        };
        Ok(WireRequest {
            path,
            body,
            headers: Vec::new(),
        })
    }

    fn decoder(&self, _req: &ChatRequest) -> Self::Decoder {
        match self.options.api {
            OpenAiApi::ChatCompletions => OpenAiDecoder::Chat(chat::ChatDecoder::default()),
            OpenAiApi::Responses => {
                OpenAiDecoder::Responses(responses::ResponsesDecoder::default())
            }
        }
    }

    fn classify(&self, status: u16, headers: &HeaderMap, body: &str) -> ProviderError {
        common::classify(status, headers, body)
    }
}

/// Ostrożne możliwości nieznanego modelu oficjalnego OpenAI (narzędzia tak; bez próbkowania —
/// modele rozumujące odrzucają `temperature`).
fn unknown_openai() -> ModelCapabilities {
    ModelCapabilities {
        tools: true,
        strict_tools: true,
        forced_tool_choice: true,
        ..ModelCapabilities::default()
    }
}

/// Dostawca OpenAI lub endpoint zgodny.
pub struct OpenAiProvider {
    engine: Arc<Engine<OpenAiCodec>>,
    registry: Arc<ModelRegistry>,
}

impl OpenAiProvider {
    /// Adapter z pełną konfiguracją.
    pub fn new(
        profile: ProviderProfile,
        http: HttpConfig,
        key: Arc<dyn SecretSource>,
        options: OpenAiOptions,
    ) -> Result<Self, ConfigError> {
        let fallback = if options.native {
            unknown_openai()
        } else {
            ModelCapabilities::default()
        };
        let registry = Arc::new(ModelRegistry::new(
            profile.models.clone(),
            |_| None,
            fallback,
        ));
        let codec = OpenAiCodec {
            options,
            profile: profile.clone(),
            registry: Arc::clone(&registry),
        };
        Ok(Self {
            engine: Arc::new(Engine::new(profile, http, key, codec)?),
            registry,
        })
    }

    /// Oficjalne OpenAI (Responses API) z `Authorization: Bearer`.
    pub fn native(
        profile: ProviderProfile,
        key: Arc<dyn SecretSource>,
    ) -> Result<Self, ConfigError> {
        let http = HttpConfig::new(OPENAI_BASE_URL, AuthScheme::Bearer);
        Self::new(profile, http, key, OpenAiOptions::native())
    }
}

#[async_trait]
impl ModelProvider for OpenAiProvider {
    fn id(&self) -> &ProviderId {
        &self.engine.profile.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.registry.provider_capabilities(&self.engine.profile)
    }

    fn stream(&self, request: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        self.engine.stream(request, cancel)
    }

    fn health(&self) -> ProviderHealth {
        self.engine.health()
    }

    fn estimate_cost(&self, request: &ChatRequest) -> Option<CostEstimate> {
        self.registry.estimate_cost(&self.engine.profile, request)
    }

    fn cost(&self, model: &str, usage: &Usage) -> Option<Cost> {
        cost(&self.engine.profile, model, usage)
    }

    /// `GET /models` (identyfikatory; możliwości ustala konfiguracja/katalog).
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let page = self.engine.request_json("/models", None).await?;
        Ok(page["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| {
                Some(ModelInfo {
                    id: m["id"].as_str()?.to_owned(),
                    display_name: m["name"].as_str().map(str::to_owned),
                    created: m["created"].as_u64().map(|c| c.to_string()),
                    capabilities: None,
                })
            })
            .collect())
    }

    /// `POST /embeddings`.
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        if let Some(caps) = self.registry.all().get(&request.model)
            && !caps.supports(ModelKind::Embeddings)
        {
            return Err(ProviderError::new(
                ProviderErrorKind::Unsupported,
                format!("`{}` nie jest modelem osadzeń", request.model),
            ));
        }
        let body = json!({"model": request.model, "input": request.input});
        let resp = self.engine.request_json("/embeddings", Some(&body)).await?;
        let mut rows: Vec<(u64, Vec<f32>)> = resp["data"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|d| {
                let v: Vec<f32> = d["embedding"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_f64)
                    .map(|x| x as f32)
                    .collect();
                (d["index"].as_u64().unwrap_or(0), v)
            })
            .collect();
        rows.sort_by_key(|(i, _)| *i);
        if rows.len() != request.input.len() {
            return Err(ProviderError::new(
                ProviderErrorKind::Protocol,
                "liczba osadzeń różna od liczby wejść",
            ));
        }
        Ok(EmbeddingResponse {
            vectors: rows.into_iter().map(|(_, v)| v).collect(),
            usage: Usage {
                input_tokens: resp["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
                ..Usage::default()
            },
        })
    }
}
