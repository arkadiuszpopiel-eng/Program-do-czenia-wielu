//! Adapter Azure OpenAI (konto firmowe, C2.12): zasób `https://<zasób>.openai.azure.com`, klucz
//! w nagłówku `api-key`, żądania na adres wdrożenia
//! `/openai/deployments/{wdrożenie}/chat/completions?api-version=…` (format Chat Completions).
//!
//! Alfa wybiera **model** (np. `gpt-4o`); adapter zamienia go na nazwę wdrożenia z konta. Model bez
//! wdrożenia idzie pod własną nazwą (wdrożenie nazwane jak model — częsta praktyka).

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use providers_contract::{
    CancellationToken, ChatRequest, Cost, CostEstimate, EmbeddingRequest, EmbeddingResponse,
    ModelInfo, ModelProvider, ProviderCapabilities, ProviderError, ProviderHealth, ProviderId,
    ProviderStream, SecretSource, Usage,
};
use reqwest::header::HeaderMap;
use serde_json::{Value, json};

use crate::openai::{embeddings_from, unknown_openai};
use crate::registry::{ModelRegistry, cost};
use lib_openai_compat::chat::{self, ChatDecoder, ChatOptions};
use lib_openai_compat::{BuildOptions, Engine, WireCodec, WireRequest};
use lib_openai_compat::{ConfigError, HttpConfig, MaxTokensField, ProviderProfile};

/// Wersja API używana, gdy konto jej nie podaje (ostatnia ogólnie dostępna).
pub const AZURE_API_VERSION: &str = "2024-10-21";

/// Ustawienia zasobu Azure OpenAI z konta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AzureOptions {
    /// Wersja API (`api-version`).
    pub api_version: String,
    /// Model Alfy → nazwa wdrożenia w zasobie.
    pub deployments: BTreeMap<String, String>,
}

impl Default for AzureOptions {
    fn default() -> Self {
        Self {
            api_version: AZURE_API_VERSION.to_owned(),
            deployments: BTreeMap::new(),
        }
    }
}

impl AzureOptions {
    /// Wdrożenie dla modelu (bez wpisu — nazwa modelu).
    pub fn deployment<'a>(&'a self, model: &'a str) -> &'a str {
        self.deployments.get(model).map_or(model, String::as_str)
    }

    /// Ścieżka operacji wdrożenia z wersją API (segmenty zakodowane — to dane z konta).
    pub fn path(&self, model: &str, operation: &str) -> String {
        format!(
            "/openai/deployments/{}/{operation}?api-version={}",
            encode(self.deployment(model)),
            encode(&self.api_version)
        )
    }

    /// Komunikat dla `404 DeploymentNotFound`: literówka w nazwie wdrożenia przechodzi test
    /// połączenia (lista modeli zasobu sprawdza tylko klucz i zasób), więc mówimy wprost, co poprawić.
    pub fn missing_deployment(&self) -> String {
        let names: Vec<&str> = self.deployments.values().map(String::as_str).collect();
        let known = match names.as_slice() {
            [] => "brak wdrożeń w koncie".to_owned(),
            list => format!("wdrożenia w koncie: {}", list.join(", ")),
        };
        format!(
            "Zasób Azure OpenAI nie ma wdrożenia o tej nazwie (DeploymentNotFound; {known}).              Sprawdź nazwę wdrożenia w portalu Azure (nowe wdrożenie bywa dostępne po kilku minutach)              albo usuń konto i dodaj je ponownie z poprawną nazwą."
        )
    }
}

/// Kodowanie procentowe wszystkiego poza znakami niezastrzeżonymi (RFC 3986).
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

pub(crate) struct AzureCodec {
    options: AzureOptions,
    profile: ProviderProfile,
    registry: Arc<ModelRegistry>,
}

impl WireCodec for AzureCodec {
    type Decoder = ChatDecoder;

    fn build(&self, req: &ChatRequest, _opts: BuildOptions) -> Result<WireRequest, ProviderError> {
        let caps = self.registry.caps(&req.model);
        let options = ChatOptions {
            max_tokens_field: MaxTokensField::MaxCompletionTokens,
            stream_usage: true,
        };
        let mut body = chat::build_body(req, &caps, &self.profile, &options)?;
        // Azure wybiera model po wdrożeniu w adresie; pole `model` niesie tę samą nazwę (jak SDK).
        if let Some(fields) = body.as_object_mut() {
            let deployment = self.options.deployment(&req.model).to_owned();
            fields.insert("model".into(), Value::String(deployment));
        }
        Ok(WireRequest {
            path: self.options.path(&req.model, "chat/completions").into(),
            body,
            headers: Vec::new(),
        })
    }

    fn decoder(&self, _req: &ChatRequest) -> Self::Decoder {
        ChatDecoder::default()
    }

    fn classify(&self, status: u16, headers: &HeaderMap, body: &str) -> ProviderError {
        let mut err = lib_openai_compat::common::classify(status, headers, body);
        if status == 404 && body.contains("DeploymentNotFound") {
            err.message = self.options.missing_deployment();
        }
        err
    }
}

/// Dostawca Azure OpenAI.
pub struct AzureOpenAiProvider {
    engine: Arc<Engine<AzureCodec>>,
    registry: Arc<ModelRegistry>,
}

impl AzureOpenAiProvider {
    /// Adapter dla zasobu (`http.base_url` = adres zasobu, `http.auth` = nagłówek `api-key`).
    pub fn new(
        profile: ProviderProfile,
        http: HttpConfig,
        key: Arc<dyn SecretSource>,
        options: AzureOptions,
    ) -> Result<Self, ConfigError> {
        // Wdrożenia to modele OpenAI — te same ostrożne możliwości co w oficjalnym API.
        let registry = Arc::new(ModelRegistry::new(
            profile.models.clone(),
            |_| None,
            unknown_openai(),
        ));
        let codec = AzureCodec {
            options,
            profile: profile.clone(),
            registry: Arc::clone(&registry),
        };
        Ok(Self {
            engine: Arc::new(Engine::new(profile, http, key, codec)?),
            registry,
        })
    }
}

#[async_trait]
impl ModelProvider for AzureOpenAiProvider {
    fn id(&self) -> &ProviderId {
        &self.engine.profile.id
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.registry.provider_capabilities(&self.engine.profile)
    }
    fn model_capabilities(&self, model: &str) -> Option<providers_contract::ModelCapabilities> {
        Some(self.registry.caps(model))
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

    /// Klucz i zasób sprawdza lista modeli zasobu (`GET /openai/models`, bez tokenów); Alfie wolno
    /// wołać tylko wdrożenia, więc wynikiem są modele wdrożeń z konta (nazwa = wdrożenie).
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let codec = &self.engine.codec;
        let path = format!(
            "/openai/models?api-version={}",
            encode(&codec.options.api_version)
        );
        self.engine.request_json(&path, None).await?;
        Ok(codec
            .options
            .deployments
            .iter()
            .map(|(model, deployment)| ModelInfo {
                id: model.clone(),
                display_name: Some(deployment.clone()),
                created: None,
                capabilities: None,
            })
            .collect())
    }

    /// `POST /openai/deployments/{wdrożenie}/embeddings?api-version=…`.
    async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        let path = self.engine.codec.options.path(&request.model, "embeddings");
        let body = json!({"input": request.input});
        let resp = self.engine.request_json(&path, Some(&body)).await?;
        embeddings_from(&resp, request.input.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_map_models_to_encoded_deployments() {
        let options = AzureOptions {
            api_version: "2025-04-01-preview".into(),
            deployments: BTreeMap::from([("gpt-4o".into(), "prod 4o/eu".into())]),
        };
        assert_eq!(options.deployment("gpt-4o"), "prod 4o/eu");
        assert_eq!(options.deployment("o3-mini"), "o3-mini");
        assert_eq!(
            options.path("gpt-4o", "chat/completions"),
            "/openai/deployments/prod%204o%2Feu/chat/completions?api-version=2025-04-01-preview"
        );
        assert_eq!(AzureOptions::default().api_version, AZURE_API_VERSION);
        assert_eq!(encode("a?b#c&d=é"), "a%3Fb%23c%26d%3D%C3%A9");
    }
}
