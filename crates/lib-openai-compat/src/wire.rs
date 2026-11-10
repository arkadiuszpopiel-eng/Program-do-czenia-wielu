//! Typy żądania i kontrakt kodeka wspólnego silnika HTTP/SSE.

use providers_contract::{ChatRequest, ProviderError, ProviderEvent};
use reqwest::header::HeaderMap;

use crate::sse::SseEvent;

/// Opcje budowy żądania dla kolejnej próby.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildOptions {
    /// Pomiń bloki myślenia (jednorazowe odzyskanie po odrzuceniu podpisu).
    pub strip_thinking: bool,
}

/// Żądanie w formacie dostawcy.
#[derive(Debug, Clone)]
pub struct WireRequest {
    /// Ścieżka względem `base_url` (z zapytaniem, np. `?api-version=…` w Azure OpenAI).
    pub path: std::borrow::Cow<'static, str>,
    /// Ciało JSON.
    pub body: serde_json::Value,
    /// Nagłówki specyficzne dla żądania (np. `anthropic-beta`).
    pub headers: Vec<(&'static str, String)>,
}

/// Dekoder zdarzeń SSE dostawcy (stan jednej odpowiedzi).
pub trait StreamDecoder: Send + 'static {
    /// Zdarzenia neutralne z jednego zdarzenia SSE (może zawierać zdarzenie końcowe).
    fn on_event(&mut self, event: SseEvent) -> Vec<ProviderEvent>;
    /// Koniec strumienia bajtów.
    fn on_eof(&mut self) -> Vec<ProviderEvent>;
}

/// Kodek formatu dostawcy.
pub trait WireCodec: Send + Sync + 'static {
    /// Typ dekodera.
    type Decoder: StreamDecoder;
    /// Buduje żądanie strumieniowe.
    fn build(&self, req: &ChatRequest, opts: BuildOptions) -> Result<WireRequest, ProviderError>;
    /// Nowy dekoder odpowiedzi.
    fn decoder(&self, req: &ChatRequest) -> Self::Decoder;
    /// Klasyfikuje odpowiedź z błędem HTTP.
    fn classify(&self, status: u16, headers: &HeaderMap, body: &str) -> ProviderError;
    /// Czy first-token/idle mierzą postęp treści zamiast porcje transportu.
    /// Domyślnie zachowuje dotychczasową semantykę API; lokalny adapter nie uznaje pingów
    /// i metadanych za generację. Treść po dekodowaniu obejmuje też narzędzia i myślenie.
    fn content_timeouts(&self) -> bool {
        false
    }
    /// Nagłówki stałe dla wszystkich żądań (np. `anthropic-version`).
    fn static_headers(&self) -> Vec<(&'static str, String)> {
        Vec::new()
    }
    /// Jednorazowe odzyskanie po błędzie (inne opcje budowy), jeśli kodek je zna.
    fn recover(&self, _err: &ProviderError, _opts: BuildOptions) -> Option<BuildOptions> {
        None
    }
}
