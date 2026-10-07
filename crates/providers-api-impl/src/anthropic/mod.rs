//! Adapter Anthropic Messages API (natywny) i generyczny „endpoint zgodny z Anthropic".

mod decode;
pub(crate) mod models;
mod request;

use std::sync::Arc;

use async_trait::async_trait;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, ModelInfo, ModelProvider,
    ProviderCapabilities, ProviderError, ProviderHealth, ProviderId, ProviderStream, SecretSource,
    Usage,
};
use reqwest::header::HeaderMap;

use crate::registry::{ModelRegistry, cost};
use lib_openai_compat::{AuthScheme, ConfigError, HttpConfig, ProviderProfile};
use lib_openai_compat::{BuildOptions, Engine, WireCodec, WireRequest};

pub use request::{
    BETA_SERVER_FALLBACK, BETA_THINKING_BINDING, BETA_THINKING_UPDATES, MID_SYSTEM_PREFIX,
};

/// Oficjalny endpoint (katalog go jeszcze nie podaje; profil użytkownika może nadpisać).
pub const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";
/// Wersja API w nagłówku `anthropic-version`.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Zachowanie przy niezgodnym prefiksie historii dla bloków myślenia (beta binding controls).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixMismatch {
    /// Błąd 400 (zmiana historii = błąd w kodzie).
    Error,
    /// API pomija unieważnione bloki.
    DropBlock,
}

impl PrefixMismatch {
    /// Wartość na drucie.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::DropBlock => "drop_block",
        }
    }
}

/// Opcje adaptera Anthropic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnthropicOptions {
    /// Oficjalne API (nagłówki beta, `fallbacks`) vs endpoint zgodny (tylko rdzeń Messages API).
    pub native: bool,
    /// Nagłówek `anthropic-version`.
    pub api_version: String,
    /// Modele, dla których wysyłamy serwerowy fallback po odmowie (`fallbacks: "default"`).
    pub server_fallback_models: Vec<String>,
    /// `eager_input_streaming` na narzędziach (argumenty strumieniowane od razu; walidacja po stronie Alfy).
    pub eager_input_streaming: bool,
    /// Jawne ustawienie wiązania bloków myślenia (`None` = domyślne konta).
    pub block_binding: Option<PrefixMismatch>,
}

impl AnthropicOptions {
    /// Oficjalne API Anthropic.
    pub fn native() -> Self {
        Self {
            native: true,
            api_version: ANTHROPIC_VERSION.into(),
            server_fallback_models: [
                "claude-opus-5-5",
                "claude-opus-5",
                "claude-sonnet-5-5",
                "claude-fable-5-1",
            ]
            .map(str::to_owned)
            .to_vec(),
            eager_input_streaming: true,
            block_binding: None,
        }
    }

    /// Endpoint zgodny z Anthropic (np. Z.ai): bez bet, bez `fallbacks`, bez `eager_input_streaming`.
    pub fn compatible() -> Self {
        Self {
            native: false,
            api_version: ANTHROPIC_VERSION.into(),
            server_fallback_models: Vec::new(),
            eager_input_streaming: false,
            block_binding: None,
        }
    }
}

pub(crate) struct AnthropicCodec {
    options: AnthropicOptions,
    profile: ProviderProfile,
    registry: Arc<ModelRegistry>,
}

impl WireCodec for AnthropicCodec {
    type Decoder = decode::AnthropicDecoder;

    fn build(&self, req: &ChatRequest, opts: BuildOptions) -> Result<WireRequest, ProviderError> {
        let caps = self.registry.caps(&req.model);
        let (body, betas) = request::build_body(
            req,
            &caps,
            &self.profile,
            &self.options,
            opts.strip_thinking,
        )?;
        let mut headers = Vec::new();
        if !betas.is_empty() {
            headers.push(("anthropic-beta", betas.join(",")));
        }
        Ok(WireRequest {
            path: "/v1/messages",
            body,
            headers,
        })
    }

    fn decoder(&self, _req: &ChatRequest) -> Self::Decoder {
        decode::AnthropicDecoder::default()
    }

    fn classify(&self, status: u16, headers: &HeaderMap, body: &str) -> ProviderError {
        decode::classify(status, headers, body)
    }

    fn static_headers(&self) -> Vec<(&'static str, String)> {
        vec![("anthropic-version", self.options.api_version.clone())]
    }

    fn recover(&self, err: &ProviderError, opts: BuildOptions) -> Option<BuildOptions> {
        (!opts.strip_thinking && decode::is_thinking_binding_error(err)).then_some(BuildOptions {
            strip_thinking: true,
        })
    }
}

/// Dostawca Anthropic (natywny lub zgodny).
pub struct AnthropicProvider {
    engine: Arc<Engine<AnthropicCodec>>,
    registry: Arc<ModelRegistry>,
}

impl AnthropicProvider {
    /// Adapter z pełną konfiguracją.
    pub fn new(
        profile: ProviderProfile,
        http: HttpConfig,
        key: Arc<dyn SecretSource>,
        options: AnthropicOptions,
    ) -> Result<Self, ConfigError> {
        let fallback = if options.native {
            models::unknown_claude()
        } else {
            providers_contract::ModelCapabilities::default()
        };
        let known = if options.native {
            models::known
        } else {
            |_: &str| None
        };
        let registry = Arc::new(ModelRegistry::new(profile.models.clone(), known, fallback));
        let codec = AnthropicCodec {
            options,
            profile: profile.clone(),
            registry: Arc::clone(&registry),
        };
        Ok(Self {
            engine: Arc::new(Engine::new(profile, http, key, codec)?),
            registry,
        })
    }

    /// Oficjalne API z domyślnym endpointem i `x-api-key`.
    pub fn native(
        profile: ProviderProfile,
        key: Arc<dyn SecretSource>,
    ) -> Result<Self, ConfigError> {
        let http = HttpConfig::new(ANTHROPIC_BASE_URL, AuthScheme::XApiKey);
        Self::new(profile, http, key, AnthropicOptions::native())
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
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

    /// `GET /v1/models` ze stronicowaniem (`after_id`, `has_more`); wykryte możliwości trafiają do rejestru.
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let mut out = Vec::new();
        let mut after: Option<String> = None;
        for _ in 0..20 {
            let path = match &after {
                Some(id) => format!("/v1/models?limit=100&after_id={id}"),
                None => "/v1/models?limit=100".to_owned(),
            };
            let page = self.engine.request_json(&path, None).await?;
            let data = page["data"].as_array().cloned().unwrap_or_default();
            out.extend(data.iter().filter_map(models::from_models_api));
            after = page["last_id"].as_str().map(str::to_owned);
            if !page["has_more"].as_bool().unwrap_or(false) || after.is_none() {
                break;
            }
        }
        self.registry.learn(&out);
        Ok(out)
    }
}
