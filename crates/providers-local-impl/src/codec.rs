//! Kodek `llama-server` (OpenAI Chat Completions) na wspólnym silniku `lib-openai-compat`.
//!
//! Żądania z narzędziami idą domyślnie w kopercie JSON ze schematem (`response_format` →
//! gramatyka serwera, [`crate::envelope`]); wyłączenie wymuszania ([`LlamaCodec::with_tool_schema`])
//! wraca do natywnego pola `tools` i formatu narzędzi szablonu czatu.

use lib_openai_compat::chat::{self, ChatDecoder, ChatOptions, MaxTokensField};
use lib_openai_compat::{BuildOptions, ProviderProfile, WireCodec, WireRequest};
use providers_contract::{ChatRequest, ModelCapabilities, ProviderError};
use reqwest::header::HeaderMap;

use crate::envelope::{self, EnvelopeGuard};
use crate::leak::LeakGuard;
use crate::manifest::ModelEntry;
use crate::window;

/// Kodek jednego modelu lokalnego.
pub struct LlamaCodec {
    caps: ModelCapabilities,
    profile: ProviderProfile,
    ctx: u32,
    tool_schema: bool,
}

impl LlamaCodec {
    /// Kodek dla wpisu manifestu i kontekstu (wymuszanie schematu wywołań narzędzi włączone).
    pub fn new(entry: &ModelEntry, ctx: u32) -> Self {
        let mut profile = ProviderProfile::new("local");
        profile.default_max_tokens = ctx.min(entry.ctx) / 2;
        Self {
            caps: entry.capabilities(ctx),
            profile,
            ctx: ctx.min(entry.ctx),
            tool_schema: true,
        }
    }

    /// Wymuszanie poprawnego JSON wywołań narzędzi schematem (C2.3); `false` — narzędzia
    /// w natywnym polu `tools`, bez koperty.
    #[must_use]
    pub fn with_tool_schema(mut self, enabled: bool) -> Self {
        self.tool_schema = enabled;
        self
    }

    /// Czy żądanie idzie w kopercie ze schematem (model z narzędziami, narzędzia w żądaniu).
    fn envelope(&self, req: &ChatRequest) -> bool {
        self.tool_schema && self.caps.tools && envelope::applies(req)
    }
}

impl WireCodec for LlamaCodec {
    type Decoder = EnvelopeGuard<LeakGuard<ChatDecoder>>;

    fn build(&self, req: &ChatRequest, _opts: BuildOptions) -> Result<WireRequest, ProviderError> {
        let options = ChatOptions {
            max_tokens_field: MaxTokensField::MaxTokens,
            stream_usage: true,
        };
        // Koperta liczona raz: rezerwa okna, prompt i `response_format` z tych samych schematów.
        let prepared = self.envelope(req).then(|| envelope::Prepared::new(req));
        // Okno rozmowy w kontekście uruchomienia (`-c`): starsze tury nie idą do serwera.
        let max_out = req
            .params
            .max_tokens
            .unwrap_or(self.profile.default_max_tokens);
        let mut reserve = window::reply_reserve(self.ctx, max_out);
        if let Some(prepared) = &prepared {
            reserve = reserve.saturating_add(prepared.overhead_tokens(req));
        }
        let fitted = window::fit(req, self.ctx, reserve);
        if fitted.dropped > 0 {
            tracing::info!(
                dropped = fitted.dropped,
                kept = fitted.request.messages.len(),
                ctx = self.ctx,
                "okno rozmowy: najstarsze wiadomości pominięte w żądaniu do llama-server"
            );
        }
        let body = if let Some(prepared) = &prepared {
            let (wire, format) = prepared.wire_request(&fitted.request);
            let mut body = chat::build_body(&wire, &self.caps, &self.profile, &options)?;
            if let Some(obj) = body.as_object_mut() {
                obj.insert("response_format".into(), format);
            }
            body
        } else {
            chat::build_body(&fitted.request, &self.caps, &self.profile, &options)?
        };
        Ok(WireRequest {
            path: "/chat/completions".into(),
            body,
            headers: Vec::new(),
        })
    }

    fn decoder(&self, req: &ChatRequest) -> Self::Decoder {
        // Bez narzędzi w żądaniu model nie ma czego wywołać — tekst w kształcie wywołania to
        // zmyślenie i nie trafia do odpowiedzi.
        let inner = LeakGuard::new(ChatDecoder::default(), req.tools.is_empty());
        let schemas = self.envelope(req).then(|| envelope::schemas(req));
        EnvelopeGuard::new(inner, schemas)
    }

    fn classify(&self, status: u16, headers: &HeaderMap, body: &str) -> ProviderError {
        lib_openai_compat::common::classify(status, headers, body)
    }
}
