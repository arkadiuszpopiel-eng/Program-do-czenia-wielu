//! Kodek `llama-server` (OpenAI Chat Completions) na wspólnym silniku `lib-openai-compat`.

use lib_openai_compat::chat::{self, ChatDecoder, ChatOptions, MaxTokensField};
use lib_openai_compat::{BuildOptions, ProviderProfile, WireCodec, WireRequest};
use providers_contract::{ChatRequest, ModelCapabilities, ProviderError};
use reqwest::header::HeaderMap;

use crate::manifest::ModelEntry;
use crate::window;

/// Kodek jednego modelu lokalnego.
pub struct LlamaCodec {
    caps: ModelCapabilities,
    profile: ProviderProfile,
    ctx: u32,
}

impl LlamaCodec {
    /// Kodek dla wpisu manifestu i kontekstu.
    pub fn new(entry: &ModelEntry, ctx: u32) -> Self {
        let mut profile = ProviderProfile::new("local");
        profile.default_max_tokens = ctx.min(entry.ctx) / 2;
        Self {
            caps: entry.capabilities(ctx),
            profile,
            ctx: ctx.min(entry.ctx),
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
        // Okno rozmowy w kontekście uruchomienia (`-c`): starsze tury nie idą do serwera.
        let max_out = req
            .params
            .max_tokens
            .unwrap_or(self.profile.default_max_tokens);
        let fitted = window::fit(req, self.ctx, window::reply_reserve(self.ctx, max_out));
        if fitted.dropped > 0 {
            tracing::info!(
                dropped = fitted.dropped,
                kept = fitted.request.messages.len(),
                ctx = self.ctx,
                "okno rozmowy: najstarsze wiadomości pominięte w żądaniu do llama-server"
            );
        }
        Ok(WireRequest {
            path: "/chat/completions",
            body: chat::build_body(&fitted.request, &self.caps, &self.profile, &options)?,
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
