//! Wspólny silnik HTTP adapterów: ponawianie przed pierwszym tokenem, limity czasu, SSE,
//! anulowanie (zadanie w tle trzyma połączenie; anulowanie zrywa je natychmiast), zdrowie.
//!
//! Adapter dostarcza [`WireCodec`] (budowa żądania, dekoder SSE, klasyfikacja błędów HTTP);
//! [`Engine`] robi resztę. Używają go `providers-api-impl` (API chmurowe) i
//! `providers-local-impl` (`llama-server` na `127.0.0.1`).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use providers_contract::{
    CancellationToken, ChatRequest, ProviderError, ProviderErrorKind, ProviderEvent,
    ProviderHealth, ProviderStream, SecretSource, StopReason, TimeoutPhase, check_privacy,
};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use tokio::sync::mpsc;

use crate::config::{ConfigError, HttpConfig, ProviderProfile};
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
    /// Ścieżka względem `base_url`.
    pub path: &'static str,
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
    /// Nagłówki stałe dla wszystkich żądań (np. `anthropic-version`).
    fn static_headers(&self) -> Vec<(&'static str, String)> {
        Vec::new()
    }
    /// Jednorazowe odzyskanie po błędzie (inne opcje budowy), jeśli kodek je zna.
    fn recover(&self, _err: &ProviderError, _opts: BuildOptions) -> Option<BuildOptions> {
        None
    }
}

#[derive(Debug, Default)]
struct Stats {
    failures: u32,
    last_error: Option<ProviderErrorKind>,
    last_ttft_ms: Option<u64>,
}

/// Silnik jednego endpointu.
pub struct Engine<C> {
    /// Profil dostawcy (tożsamość, prywatność, cennik).
    pub profile: ProviderProfile,
    /// Endpoint, uwierzytelnienie, limity czasu, ponawianie.
    pub http: HttpConfig,
    /// Źródło klucza (odczyt w chwili wywołania).
    pub key: Arc<dyn SecretSource>,
    /// Kodek formatu dostawcy.
    pub codec: C,
    pub(crate) client: reqwest::Client,
    stats: Mutex<Stats>,
}

/// Strumień z jednym zdarzeniem.
pub fn single(event: ProviderEvent) -> ProviderStream {
    Box::pin(futures_util::stream::iter([event]))
}

/// Zdarzenie końcowe anulowania.
pub fn cancelled() -> ProviderEvent {
    ProviderEvent::stop(StopReason::Cancelled)
}

/// Maksymalna długość komunikatu błędu dostawcy przekazywanego dalej.
const MAX_ERROR_MESSAGE: usize = 500;

impl<C: WireCodec> Engine<C> {
    /// Silnik z klientem HTTP (limit połączenia z `http.timeouts.connect`).
    pub fn new(
        profile: ProviderProfile,
        http: HttpConfig,
        key: Arc<dyn SecretSource>,
        codec: C,
    ) -> Result<Self, ConfigError> {
        let client = reqwest::Client::builder()
            .connect_timeout(http.timeouts.connect)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .map_err(|e| ConfigError::Http(e.to_string()))?;
        Ok(Self {
            profile,
            http,
            key,
            codec,
            client,
            stats: Mutex::default(),
        })
    }

    fn stats(&self) -> std::sync::MutexGuard<'_, Stats> {
        self.stats.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Zdrowie z historii wywołań (`Unconfigured`, gdy brak wymaganego klucza).
    pub fn health(&self) -> ProviderHealth {
        if self.http.auth.requires_key() && self.key.api_key().is_none_or(|k| k.is_empty()) {
            return ProviderHealth {
                state: providers_contract::HealthState::Unconfigured,
                ..ProviderHealth::healthy()
            };
        }
        let st = self.stats();
        ProviderHealth::from_stats(st.failures, st.last_error.clone(), st.last_ttft_ms, 3)
    }

    fn record(&self, terminal: &ProviderEvent, ttft: Option<Duration>) {
        let mut st = self.stats();
        match terminal {
            // Błędy po naszej stronie (walidacja, brak możliwości) nie obniżają zdrowia dostawcy.
            ProviderEvent::Error(e)
                if e.should_fallback() && e.kind != ProviderErrorKind::Unsupported =>
            {
                st.failures = st.failures.saturating_add(1);
                st.last_error = Some(e.kind.clone());
            }
            ProviderEvent::Stop { reason, .. } if *reason != StopReason::Cancelled => {
                st.failures = 0;
                st.last_ttft_ms = ttft.map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
            }
            _ => {}
        }
    }

    /// Strumień odpowiedzi; walidacja i prywatność przed jakimkolwiek ruchem sieciowym.
    pub fn stream(self: &Arc<Self>, req: ChatRequest, cancel: CancellationToken) -> ProviderStream {
        if cancel.is_cancelled() {
            return single(cancelled());
        }
        if let Err(e) = req
            .validate()
            .and_then(|()| check_privacy(&req.meta.privacy, &self.profile.privacy))
        {
            return single(ProviderEvent::Error(e));
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return single(ProviderEvent::Error(ProviderError::new(
                ProviderErrorKind::Protocol,
                "adapter wymaga środowiska tokio",
            )));
        };
        let (tx, rx) = mpsc::channel(64);
        let engine = Arc::clone(self);
        runtime.spawn(async move {
            let mut run = crate::run::Run::new(&engine, &tx, &cancel);
            if let Some(terminal) = run.execute(&req).await {
                engine.record(&terminal, run.ttft);
                let _ = tx.send(terminal).await;
            }
        });
        Box::pin(tokio_stream::wrappers::ReceiverStream::new(rx))
    }

    pub(crate) fn headers(
        &self,
        extra: &[(&'static str, String)],
    ) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();
        let key = self.key.api_key();
        self.http.auth.apply(&mut headers, key.as_ref())?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let statics = self.codec.static_headers();
        let pairs = statics
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .chain(self.http.extra_headers.iter().cloned())
            .chain(extra.iter().map(|(k, v)| ((*k).to_owned(), v.clone())));
        for (name, value) in pairs {
            let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                ProviderError::invalid_request(format!("zła nazwa nagłówka `{name}`"))
            })?;
            let value = HeaderValue::from_str(&value).map_err(|_| {
                ProviderError::invalid_request(format!("zła wartość nagłówka `{name}`"))
            })?;
            headers.insert(name, value);
        }
        Ok(headers)
    }

    pub(crate) fn scrub(&self, mut err: ProviderError) -> ProviderError {
        if let Some(key) = self.key.api_key() {
            err.message = key.redact_in(&err.message);
        }
        if err.message.chars().count() > MAX_ERROR_MESSAGE {
            err.message = err
                .message
                .chars()
                .take(MAX_ERROR_MESSAGE)
                .collect::<String>()
                + "…";
        }
        err
    }

    pub(crate) async fn error_from_response(&self, resp: reqwest::Response) -> ProviderError {
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let body = tokio::time::timeout(Duration::from_secs(2), resp.text())
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or_default();
        self.scrub(self.codec.classify(status, &headers, &body))
    }

    /// Żądanie JSON bez strumienia (Models API, osadzenia).
    pub async fn request_json(
        &self,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ProviderError> {
        let headers = self.headers(&[])?;
        let url = self.http.url(path);
        let builder = match body {
            Some(b) => self.client.post(url).body(to_bytes(b)?),
            None => self.client.get(url),
        };
        let limit = self.http.timeouts.first_token;
        let resp = tokio::time::timeout(limit, builder.headers(headers).send())
            .await
            .map_err(|_| timeout_err(TimeoutPhase::FirstToken))?
            .map_err(|e| self.scrub(classify_transport(&e)))?;
        if !resp.status().is_success() {
            return Err(self.error_from_response(resp).await);
        }
        let bytes = tokio::time::timeout(self.http.timeouts.idle, resp.bytes())
            .await
            .map_err(|_| timeout_err(TimeoutPhase::Idle))?
            .map_err(|e| self.scrub(classify_transport(&e)))?;
        serde_json::from_slice(&bytes).map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::Protocol,
                format!("niepoprawny JSON: {e}"),
            )
        })
    }
}

pub(crate) fn to_bytes(body: &serde_json::Value) -> Result<Vec<u8>, ProviderError> {
    serde_json::to_vec(body)
        .map_err(|e| ProviderError::invalid_request(format!("serializacja żądania: {e}")))
}

pub(crate) fn timeout_err(phase: TimeoutPhase) -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Timeout { phase },
        "przekroczony limit czasu odpowiedzi dostawcy",
    )
}

/// Klasyfikacja błędów transportu `reqwest`.
pub(crate) fn classify_transport(err: &reqwest::Error) -> ProviderError {
    let kind = if err.is_connect() && err.is_timeout() {
        ProviderErrorKind::Timeout {
            phase: TimeoutPhase::Connect,
        }
    } else if err.is_timeout() {
        ProviderErrorKind::Timeout {
            phase: TimeoutPhase::FirstToken,
        }
    } else {
        ProviderErrorKind::Network
    };
    ProviderError::new(kind, format!("transport: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthScheme;
    use providers_contract::StaticKey;

    struct Nop;
    struct NopDecoder;
    impl StreamDecoder for NopDecoder {
        fn on_event(&mut self, _event: SseEvent) -> Vec<ProviderEvent> {
            vec![]
        }
        fn on_eof(&mut self) -> Vec<ProviderEvent> {
            vec![]
        }
    }
    impl WireCodec for Nop {
        type Decoder = NopDecoder;
        fn build(
            &self,
            _req: &ChatRequest,
            _o: BuildOptions,
        ) -> Result<WireRequest, ProviderError> {
            Err(ProviderError::invalid_request("nop"))
        }
        fn decoder(&self, _req: &ChatRequest) -> NopDecoder {
            NopDecoder
        }
        fn classify(&self, status: u16, _h: &HeaderMap, _b: &str) -> ProviderError {
            ProviderError::new(ProviderErrorKind::Server { status }, "x")
        }
    }

    fn engine(extra: Vec<(String, String)>) -> Engine<Nop> {
        let mut http = HttpConfig::new("http://127.0.0.1:9", AuthScheme::Bearer);
        http.extra_headers = extra;
        Engine::new(
            ProviderProfile::new("p"),
            http,
            Arc::new(StaticKey::new("sk-long-secret")),
            Nop,
        )
        .unwrap()
    }

    #[test]
    fn headers_scrub_and_no_runtime() {
        let e = engine(vec![("x-title".into(), "Alfa".into())]);
        let h = e.headers(&[("anthropic-beta", "b1".into())]).unwrap();
        assert_eq!(h.get("x-title").and_then(|v| v.to_str().ok()), Some("Alfa"));
        assert_eq!(
            h.get("anthropic-beta").and_then(|v| v.to_str().ok()),
            Some("b1")
        );
        assert!(
            engine(vec![("zły nagłówek".into(), "v".into())])
                .headers(&[])
                .is_err()
        );
        assert!(
            engine(vec![("x-a".into(), "\n".into())])
                .headers(&[])
                .is_err()
        );
        let long = format!("sk-long-secret {}", "x".repeat(600));
        let scrubbed = e.scrub(ProviderError::new(ProviderErrorKind::Network, long));
        assert!(scrubbed.message.starts_with("[REDACTED]"));
        assert_eq!(scrubbed.message.chars().count(), MAX_ERROR_MESSAGE + 1);
        // Poza środowiskiem tokio: błąd zamiast paniki.
        let req = ChatRequest::new("m", vec![providers_contract::Message::user_text("x")]);
        let mut s = Arc::new(e).stream(req, CancellationToken::new());
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        let ev = futures_util::Stream::poll_next(s.as_mut(), &mut cx);
        assert!(matches!(
            ev,
            std::task::Poll::Ready(Some(ProviderEvent::Error(err))) if err.kind == ProviderErrorKind::Protocol
        ));
    }

    #[tokio::test]
    async fn request_json_classifies_transport_errors() {
        let e = engine(vec![]);
        let err = e.request_json("/models", None).await.unwrap_err();
        assert!(matches!(
            err.kind,
            ProviderErrorKind::Network | ProviderErrorKind::Timeout { .. }
        ));
        let body = serde_json::json!({"a": 1});
        assert!(e.request_json("/x", Some(&body)).await.is_err());
    }
}
