//! Kodek `llama-server` (OpenAI Chat Completions) na wspólnym silniku `lib-openai-compat`.

use lib_openai_compat::chat::{self, ChatDecoder, ChatOptions, MaxTokensField};
use lib_openai_compat::{BuildOptions, ProviderProfile, WireCodec, WireRequest};
use providers_contract::{ChatRequest, ModelCapabilities, ProviderError};
use reqwest::header::HeaderMap;

use crate::manifest::ModelEntry;

/// Kodek jednego modelu lokalnego.
pub struct LlamaCodec {
    caps: ModelCapabilities,
    profile: ProviderProfile,
}

impl LlamaCodec {
    /// Kodek dla wpisu manifestu i kontekstu.
    pub fn new(entry: &ModelEntry, ctx: u32) -> Self {
        let mut profile = ProviderProfile::new("local");
        profile.default_max_tokens = ctx.min(entry.ctx) / 2;
        Self {
            caps: entry.capabilities(ctx),
            profile,
        }
    }
}

impl WireCodec for LlamaCodec {
    type Decoder = ChatDecoder;

    fn build(&self, req: &ChatRequest, _opts: BuildOptions) -> Result<WireRequest, ProviderError> {
        let options = ChatOptions {
            max_tokens_field: MaxTokensField::MaxTokens,
            stream_usage: true,
        };
        Ok(WireRequest {
            path: "/chat/completions",
            body: chat::build_body(req, &self.caps, &self.profile, &options)?,
            headers: Vec::new(),
        })
    }

    fn decoder(&self, _req: &ChatRequest) -> ChatDecoder {
        ChatDecoder::default()
    }

    fn classify(&self, status: u16, headers: &HeaderMap, body: &str) -> ProviderError {
        lib_openai_compat::common::classify(status, headers, body)
    }
}
