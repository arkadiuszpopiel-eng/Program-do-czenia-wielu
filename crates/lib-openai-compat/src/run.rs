//! Jedno wykonanie żądania strumieniowego w zadaniu tła: ponowienia przed pierwszym tokenem,
//! limity czasu, parsowanie SSE, przekazywanie zdarzeń z obsługą anulowania i zniknięcia odbiorcy.

use std::time::Duration;

use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, ProviderError, ProviderErrorKind, ProviderEvent, TimeoutPhase,
};
use reqwest::header::{ACCEPT, HeaderValue};
use tokio::sync::mpsc;
use tokio::time::{Instant, timeout_at};

use crate::engine::{
    BuildOptions, Engine, StreamDecoder, WireCodec, cancelled, classify_transport, timeout_err,
    to_bytes,
};
use crate::sse::SseParser;

pub(crate) enum Failure {
    Cancelled,
    /// Odbiorca upuścił strumień — kończymy bez zdarzenia (połączenie zostaje zerwane).
    Gone,
    Failed(ProviderError),
}

/// Jedno wykonanie żądania (w zadaniu tła).
pub(crate) struct Run<'a, C> {
    engine: &'a Engine<C>,
    tx: &'a mpsc::Sender<ProviderEvent>,
    cancel: &'a CancellationToken,
    started: Instant,
    first_deadline: Instant,
    emitted: bool,
    pub(crate) ttft: Option<Duration>,
}

impl<'a, C: WireCodec> Run<'a, C> {
    pub(crate) fn new(
        engine: &'a Engine<C>,
        tx: &'a mpsc::Sender<ProviderEvent>,
        cancel: &'a CancellationToken,
    ) -> Self {
        let now = Instant::now();
        Self {
            engine,
            tx,
            cancel,
            started: now,
            first_deadline: now + engine.http.timeouts.first_token,
            emitted: false,
            ttft: None,
        }
    }

    /// Zwraca zdarzenie końcowe albo `None`, gdy odbiorca zniknął.
    pub(crate) async fn execute(&mut self, req: &ChatRequest) -> Option<ProviderEvent> {
        match self.connect(req).await {
            Ok(resp) => self.pump(resp, req).await,
            Err(Failure::Cancelled) => Some(cancelled()),
            Err(Failure::Gone) => None,
            Err(Failure::Failed(e)) => Some(ProviderEvent::Error(e)),
        }
    }

    async fn connect(&mut self, req: &ChatRequest) -> Result<reqwest::Response, Failure> {
        let retry = self.engine.http.retry;
        let mut opts = BuildOptions::default();
        let mut attempt = 0u32;
        let mut recovered = false;
        loop {
            let err = match self.attempt(req, opts).await {
                Ok(resp) => return Ok(resp),
                Err(Failure::Failed(err)) => err,
                Err(other) => return Err(other),
            };
            if !recovered && let Some(next) = self.engine.codec.recover(&err, opts) {
                tracing::warn!(provider = %self.engine.profile.id, "odrzucony podpis myślenia — jednorazowe ponowienie bez bloków myślenia");
                opts = next;
                recovered = true;
                continue;
            }
            attempt += 1;
            let retry_after = err.kind.retry_after_ms().map(Duration::from_millis);
            match retry.delay(attempt, retry_after) {
                Some(d)
                    if err.is_retryable() && retry.allows(attempt, self.started.elapsed(), d) =>
                {
                    tracing::debug!(provider = %self.engine.profile.id, attempt, delay_ms = d.as_millis(), kind = %err.kind, "ponawiam żądanie");
                    tokio::select! {
                        biased;
                        () = self.cancel.cancelled() => return Err(Failure::Cancelled),
                        () = self.tx.closed() => return Err(Failure::Gone),
                        () = tokio::time::sleep(d) => {}
                    }
                }
                _ => return Err(Failure::Failed(err)),
            }
        }
    }

    async fn attempt(
        &mut self,
        req: &ChatRequest,
        opts: BuildOptions,
    ) -> Result<reqwest::Response, Failure> {
        let engine = self.engine;
        let wire = engine.codec.build(req, opts).map_err(Failure::Failed)?;
        let mut headers = engine.headers(&wire.headers).map_err(Failure::Failed)?;
        headers.insert(ACCEPT, HeaderValue::from_static("text/event-stream"));
        let body = to_bytes(&wire.body).map_err(Failure::Failed)?;
        let send = engine
            .client
            .post(engine.http.url(wire.path))
            .headers(headers)
            .body(body)
            .send();
        self.first_deadline = Instant::now() + engine.http.timeouts.first_token;
        let res = tokio::select! {
            biased;
            () = self.cancel.cancelled() => return Err(Failure::Cancelled),
            () = self.tx.closed() => return Err(Failure::Gone),
            r = timeout_at(self.first_deadline, send) => r,
        };
        match res {
            Err(_) => Err(Failure::Failed(timeout_err(TimeoutPhase::FirstToken))),
            Ok(Err(e)) => Err(Failure::Failed(engine.scrub(classify_transport(&e)))),
            Ok(Ok(resp)) if resp.status().is_success() => Ok(resp),
            Ok(Ok(resp)) => Err(Failure::Failed(engine.error_from_response(resp).await)),
        }
    }

    async fn pump(&mut self, resp: reqwest::Response, req: &ChatRequest) -> Option<ProviderEvent> {
        let idle = self.engine.http.timeouts.idle;
        let mut body = resp.bytes_stream();
        let mut parser = SseParser::new();
        let mut decoder = self.engine.codec.decoder(req);
        let mut deadline = self.first_deadline;
        let mut seen = false;
        loop {
            let next = tokio::select! {
                biased;
                () = self.cancel.cancelled() => return Some(cancelled()),
                () = self.tx.closed() => return None,
                r = timeout_at(deadline, body.next()) => r,
            };
            let (batch, eof) = match next {
                Err(_) => {
                    let phase = if seen {
                        TimeoutPhase::Idle
                    } else {
                        TimeoutPhase::FirstToken
                    };
                    return Some(self.fail(timeout_err(phase)));
                }
                Ok(Some(Err(e))) => {
                    let err = ProviderError::new(
                        ProviderErrorKind::Network,
                        format!("urwany strumień: {e}"),
                    );
                    return Some(self.fail(err));
                }
                Ok(Some(Ok(bytes))) => {
                    let evs: Vec<ProviderEvent> = parser
                        .push(&bytes)
                        .into_iter()
                        .flat_map(|e| decoder.on_event(e))
                        .collect();
                    (evs, false)
                }
                Ok(None) => {
                    let mut evs: Vec<ProviderEvent> = parser
                        .finish()
                        .into_iter()
                        .flat_map(|e| decoder.on_event(e))
                        .collect();
                    evs.extend(decoder.on_eof());
                    (evs, true)
                }
            };
            seen = true;
            deadline = Instant::now() + idle;
            for ev in batch {
                match self.forward(ev).await {
                    Forward::Continue => {}
                    Forward::Terminal(t) => return Some(t),
                    Forward::Gone => return None,
                }
            }
            if eof {
                let err = ProviderError::new(
                    ProviderErrorKind::Protocol,
                    "strumień zakończony bez zdarzenia końcowego",
                );
                return Some(self.fail(err));
            }
        }
    }

    fn fail(&self, err: ProviderError) -> ProviderEvent {
        let after = self.emitted || err.after_output;
        ProviderEvent::Error(err.after_output(after))
    }

    async fn forward(&mut self, ev: ProviderEvent) -> Forward {
        if let ProviderEvent::Error(e) = ev {
            return Forward::Terminal(self.fail(e));
        }
        if ev.is_terminal() {
            return Forward::Terminal(ev);
        }
        if ev.is_content() && !self.emitted {
            self.emitted = true;
            self.ttft = Some(self.started.elapsed());
        }
        tokio::select! {
            biased;
            () = self.cancel.cancelled() => Forward::Terminal(cancelled()),
            r = self.tx.send(ev) => if r.is_ok() { Forward::Continue } else { Forward::Gone },
        }
    }
}

enum Forward {
    Continue,
    Terminal(ProviderEvent),
    Gone,
}
