//! Strumień z fallbackiem: próby kolejnych celów decyzji z tą samą historią.
//!
//! - błąd z `should_fallback()` **przed pierwszą treścią** albo brak pierwszego zdarzenia w terminie
//!   klasy → natychmiastowe przełączenie na następny cel (ACC F1-04: ≤ 2 s, 0 utraconych
//!   wiadomości — kolejny cel dostaje identyczne żądanie);
//! - zdarzenia przed pierwszą treścią (`Started`, `Usage`) są buforowane, więc konsument widzi
//!   jedno `Started` — od celu, który faktycznie odpowiada;
//! - błąd **po** treści (`after_output`) nie jest maskowany: konsument dostaje błąd, a złożona tura
//!   zawiera częściową odpowiedź;
//! - każdy wynik trafia do obwodów/okien limitów (`Router::report`).

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use providers_contract::{
    CancellationToken, ChatRequest, ProviderError, ProviderErrorKind, ProviderEvent, ProviderId,
    ProviderStream, StopReason,
};
use router_contract::{
    Candidate, FallbackCause, Outcome, RouteDecision, Router, RouterEvent, TaskClass,
};
use tokio::sync::mpsc;
use tokio::time::{Instant, timeout_at};

use crate::history::{localize_history, qualify_event};
use crate::routing::RouterCore;

/// Czy dostawca odrzucił sam model (nie istnieje, wycofany, konto nie ma do niego dostępu).
pub(crate) fn model_unavailable(e: &providers_contract::ProviderError) -> bool {
    if e.kind != ProviderErrorKind::InvalidRequest {
        return false;
    }
    let code = e.provider_code.as_deref().unwrap_or_default();
    let text = e.message.to_lowercase();
    matches!(
        code,
        "model_not_found" | "not_found_error" | "unsupported_model"
    ) || (text.contains("model")
        && (text.contains("does not exist")
            || text.contains("not found")
            || text.contains("do not have access")
            || text.contains("is not supported")
            || text.contains("deprecated")))
        // Model bez narzędzi (OpenRouter: „No endpoints found that support tool use”) — następny
        // model z kolejki może je mieć; katalog zakłada narzędzia u dostawcy (przegląd 2026-10-21).
        || ((text.contains("tool") || text.contains("function call"))
            && (text.contains("not support")
                || text.contains("support tool")
                || text.contains("unsupported")))
}

/// Czy żądanie nie zmieściło się w oknie kontekstu tego modelu (llama-server:
/// `exceed_context_size_error`, OpenAI: `context_length_exceeded`). Dotyczy modelu, nie treści —
/// model z większym oknem (zwykle w chmurze) ma szansę, więc przełączamy mimo kodu 400. Wcześniej
/// długa pętla narzędzi przeniesiona z chmury na mały model lokalny kończyła się ogólnym błędem
/// „nieprawidłowe żądanie” (przegląd wymiany informacji 1.11).
pub(crate) fn context_overflow(e: &providers_contract::ProviderError) -> bool {
    if e.kind != ProviderErrorKind::InvalidRequest {
        return false;
    }
    let code = e.provider_code.as_deref().unwrap_or_default();
    let text = e.message.to_lowercase();
    matches!(
        code,
        "context_length_exceeded" | "exceed_context_size_error" | "string_above_max_length"
    ) || (text.contains("context")
        && (text.contains("exceed")
            || text.contains("too long")
            || text.contains("maximum context")
            || text.contains("context length")
            || text.contains("context size")))
}

fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Wynik jednej próby.
enum Attempt {
    /// Zakończono (zdarzenie końcowe przekazane albo odbiorca zniknął).
    Done,
    /// Przełącz na następny cel.
    Next(FallbackCause),
}

pub(crate) struct Run {
    pub core: Arc<RouterCore>,
    pub router_id: ProviderId,
    pub class: TaskClass,
    pub decision: RouteDecision,
    pub request: ChatRequest,
    pub cancel: CancellationToken,
    pub tx: mpsc::Sender<ProviderEvent>,
}

impl Run {
    pub(crate) async fn execute(self) {
        let started = Instant::now();
        let targets: Vec<Candidate> = self.decision.targets().cloned().collect();
        let deadline = self.core.policy().first_event_deadline_for(
            self.class,
            providers_contract::estimate_input_tokens(&self.request),
        );
        let history = localize_history(&self.request.messages, &self.router_id);
        for (i, cand) in targets.iter().enumerate() {
            if self.cancel.is_cancelled() {
                let _ = self
                    .tx
                    .send(ProviderEvent::stop(StopReason::Cancelled))
                    .await;
                return;
            }
            let next = targets.get(i + 1);
            let Some(reg) = self.core.registered(&cand.provider) else {
                continue;
            };
            let mut req = self.request.clone();
            req.model.clone_from(&cand.model);
            req.messages.clone_from(&history);
            self.core.begin_attempt(cand);
            let child = self.cancel.child_token();
            let inner = reg.provider.stream(req, child.clone());
            let limit = deadline.filter(|_| next.is_some());
            match self.attempt(cand, inner, limit, next.is_some()).await {
                Attempt::Done => {
                    child.cancel();
                    return;
                }
                Attempt::Next(cause) => {
                    child.cancel();
                    if let Some(to) = next {
                        tracing::warn!(from = %cand, %to, ?cause, "fallback Routera");
                        self.core.emit(RouterEvent::Fallback {
                            class: self.class,
                            from: cand.clone(),
                            to: to.clone(),
                            cause,
                            elapsed_ms: ms(started.elapsed()),
                        });
                    }
                }
            }
        }
        // Cele wyczerpane bez zdarzenia końcowego (np. wyrejestrowane w trakcie).
        let err = ProviderError::new(
            ProviderErrorKind::Unsupported,
            "router: żaden cel nie jest już dostępny",
        );
        let _ = self.tx.send(ProviderEvent::Error(err)).await;
    }

    async fn attempt(
        &self,
        cand: &Candidate,
        mut inner: ProviderStream,
        limit: Option<Duration>,
        has_next: bool,
    ) -> Attempt {
        let t0 = Instant::now();
        let mut buffered: Vec<ProviderEvent> = Vec::new();
        let mut first_event = false;
        let mut content = false;
        let mut ttft: Option<Duration> = None;
        loop {
            let limit_now = limit.filter(|_| !first_event);
            let wait = async {
                match limit_now {
                    Some(d) => timeout_at(t0 + d, inner.next()).await.map_err(|_| d),
                    None => Ok(inner.next().await),
                }
            };
            let waited = tokio::select! {
                biased;
                () = self.cancel.cancelled() => {
                    // Router kończy własną próbę niezależnie od reakcji adaptera na token.
                    self.core.report(cand, Outcome::Cancelled);
                    let _ = self.tx.send(ProviderEvent::stop(StopReason::Cancelled)).await;
                    return Attempt::Done;
                }
                () = self.tx.closed() => {
                    // Konsument upuścił strumień — przerywamy (wywołujący anuluje cel).
                    self.core.report(cand, Outcome::Cancelled);
                    return Attempt::Done;
                }
                r = wait => r,
            };
            let next = match waited {
                Ok(ev) => ev,
                Err(d) => {
                    // Liczy się do obwodu: dostawca, który stale milczy, ma zostać odcięty. Wolny
                    // start długiego promptu nie przekracza terminu — ten rośnie z długością
                    // kontekstu (`first_event_deadline_for`; przegląd 2026-10-21).
                    let kind = ProviderErrorKind::Timeout {
                        phase: providers_contract::TimeoutPhase::FirstToken,
                    };
                    self.core.report(cand, Outcome::Failed { kind });
                    return Attempt::Next(FallbackCause::Deadline { deadline_ms: ms(d) });
                }
            };
            first_event = true;
            let ev = next.unwrap_or_else(|| {
                ProviderEvent::Error(
                    ProviderError::new(
                        ProviderErrorKind::Protocol,
                        "strumień zakończony bez zdarzenia końcowego",
                    )
                    .after_output(content),
                )
            });
            let ev = qualify_event(ev, cand);
            if !content && ev.is_content() {
                content = true;
                ttft = Some(t0.elapsed());
                for b in buffered.drain(..) {
                    if self.tx.send(b).await.is_err() {
                        return Attempt::Done;
                    }
                }
            }
            if !ev.is_terminal() {
                if content {
                    if self.tx.send(ev).await.is_err() {
                        self.core.report(cand, Outcome::Cancelled);
                        return Attempt::Done;
                    }
                } else {
                    buffered.push(ev);
                }
                continue;
            }
            return self
                .finish(cand, ev, buffered, content, has_next, t0, ttft)
                .await;
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn finish(
        &self,
        cand: &Candidate,
        ev: ProviderEvent,
        buffered: Vec<ProviderEvent>,
        content: bool,
        has_next: bool,
        t0: Instant,
        ttft: Option<Duration>,
    ) -> Attempt {
        let ev = match ev {
            ProviderEvent::Error(e) => {
                let after = e.after_output || content;
                let e = e.after_output(after);
                self.core.report(
                    cand,
                    Outcome::Failed {
                        kind: e.kind.clone(),
                    },
                );
                // „Nie ma takiego modelu / brak dostępu” i „za długie dla tego okna” dotyczą tego
                // jednego modelu, nie treści żądania — następny z kolejki ma szansę mimo kodu 400/404.
                let switch = !e.after_output
                    && (e.should_fallback() || model_unavailable(&e) || context_overflow(&e))
                    && has_next
                    && !self.cancel.is_cancelled();
                if switch {
                    return Attempt::Next(FallbackCause::Error { kind: e.kind });
                }
                ProviderEvent::Error(e)
            }
            ProviderEvent::Stop { reason, details } => {
                let outcome = if reason == StopReason::Cancelled {
                    Outcome::Cancelled
                } else {
                    Outcome::Ok {
                        ttft_ms: ttft.map(ms),
                        latency_ms: ms(t0.elapsed()),
                    }
                };
                self.core.report(cand, outcome);
                ProviderEvent::Stop { reason, details }
            }
            other => other,
        };
        if !content {
            // Błąd przed `Started`: konsument dowiaduje się, który cel zawiódł (komunikat
            // „limit zapytań u dostawcy …”, konto z problemem) — wcześniej widział „Router”
            // i obwiniał konto z pierwszego wyboru, choć zawiódł zapas (przegląd 2026-10-21).
            let started = buffered
                .iter()
                .any(|b| matches!(b, ProviderEvent::Started { .. }));
            // Odmowa prywatności: żądanie nie wyszło z Alfy — bez „odpowiada …” na osi czasu.
            let sent = !matches!(&ev, ProviderEvent::Error(e) if e.kind == ProviderErrorKind::PrivacyBlocked);
            if matches!(ev, ProviderEvent::Error(_)) && !started && sent {
                let origin = ProviderEvent::Started {
                    model: cand.qualified(),
                    response_id: None,
                };
                if self.tx.send(origin).await.is_err() {
                    return Attempt::Done;
                }
            }
            for b in buffered {
                if self.tx.send(b).await.is_err() {
                    return Attempt::Done;
                }
            }
        }
        let _ = self.tx.send(ev).await;
        Attempt::Done
    }
}
