//! Odtwarzanie skryptu jako `ProviderStream` z anulowaniem i limitami czasu.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use providers_contract::{
    CancellationToken, ProviderError, ProviderErrorKind, ProviderEvent, ProviderStream, StopReason,
    TimeoutPhase,
};
use tokio::time::Instant;

use crate::script::{Script, Step};

/// Limity czasu symulowane przez atrapę (jak w adapterach HTTP).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FakeTimeouts {
    /// Maksymalny czas do pierwszego zdarzenia.
    pub first_token: Option<Duration>,
    /// Maksymalna przerwa między zdarzeniami.
    pub idle: Option<Duration>,
}

/// Statystyki wywołań (dla `health`).
#[derive(Debug, Default)]
pub(crate) struct Stats {
    pub consecutive_failures: u32,
    pub last_error: Option<ProviderErrorKind>,
    pub last_ttft_ms: Option<u64>,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

struct Player {
    steps: VecDeque<Step>,
    cancel: CancellationToken,
    timeouts: FakeTimeouts,
    stats: Arc<Mutex<Stats>>,
    started_at: Instant,
    seen_event: bool,
    emitted_content: bool,
    done: bool,
}

enum Wait {
    Elapsed,
    Cancelled,
    TimedOut(TimeoutPhase),
}

impl Player {
    fn limit(&self) -> Option<(Duration, TimeoutPhase)> {
        if self.seen_event {
            self.timeouts.idle.map(|d| (d, TimeoutPhase::Idle))
        } else {
            self.timeouts
                .first_token
                .map(|d| (d, TimeoutPhase::FirstToken))
        }
    }

    async fn wait(&self, duration: Option<Duration>) -> Wait {
        let limit = self.limit();
        let (sleep_for, on_expiry) = match (duration, limit) {
            (Some(d), Some((l, phase))) if l < d => (Some(l), Wait::TimedOut(phase)),
            (Some(d), _) => (Some(d), Wait::Elapsed),
            (None, Some((l, phase))) => (Some(l), Wait::TimedOut(phase)),
            (None, None) => (None, Wait::Elapsed),
        };
        match sleep_for {
            Some(d) => tokio::select! {
                () = self.cancel.cancelled() => Wait::Cancelled,
                () = tokio::time::sleep(d) => on_expiry,
            },
            None => {
                self.cancel.cancelled().await;
                Wait::Cancelled
            }
        }
    }

    fn finish(&mut self, event: ProviderEvent) -> ProviderEvent {
        self.done = true;
        let mut st = lock(&self.stats);
        match &event {
            ProviderEvent::Error(e) => {
                st.consecutive_failures += 1;
                st.last_error = Some(e.kind.clone());
            }
            ProviderEvent::Stop { reason, .. } if *reason != StopReason::Cancelled => {
                st.consecutive_failures = 0;
            }
            _ => {}
        }
        event
    }

    fn emit(&mut self, event: ProviderEvent) -> ProviderEvent {
        self.seen_event = true;
        if event.is_content() && !self.emitted_content {
            self.emitted_content = true;
            let ms = u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
            lock(&self.stats).last_ttft_ms = Some(ms);
        }
        match event {
            ProviderEvent::Error(e) => {
                let after = self.emitted_content;
                self.finish(ProviderEvent::Error(e.after_output(after)))
            }
            ev if ev.is_terminal() => self.finish(ev),
            ev => ev,
        }
    }

    async fn next(&mut self) -> Option<ProviderEvent> {
        loop {
            if self.done {
                return None;
            }
            if self.cancel.is_cancelled() {
                return Some(self.finish(ProviderEvent::stop(StopReason::Cancelled)));
            }
            let Some(step) = self.steps.pop_front() else {
                let err = ProviderError::new(
                    ProviderErrorKind::Protocol,
                    "atrapa: skrypt bez zdarzenia końcowego",
                )
                .after_output(self.emitted_content);
                return Some(self.finish(ProviderEvent::Error(err)));
            };
            let wait = match step {
                Step::Emit(ev) => return Some(self.emit(ev)),
                Step::Delay(d) => self.wait(Some(d)).await,
                Step::Stall => self.wait(None).await,
            };
            match wait {
                Wait::Elapsed => {}
                Wait::Cancelled => {
                    return Some(self.finish(ProviderEvent::stop(StopReason::Cancelled)));
                }
                Wait::TimedOut(phase) => {
                    let err = ProviderError::new(
                        ProviderErrorKind::Timeout { phase },
                        "atrapa: przekroczony limit czasu",
                    )
                    .after_output(self.emitted_content);
                    return Some(self.finish(ProviderEvent::Error(err)));
                }
            }
        }
    }
}

/// Strumień z jednym zdarzeniem (np. błąd walidacji).
pub(crate) fn single(event: ProviderEvent) -> ProviderStream {
    Box::pin(futures_util::stream::iter([event]))
}

/// Odtwarza skrypt.
pub(crate) fn play(
    script: Script,
    cancel: CancellationToken,
    timeouts: FakeTimeouts,
    stats: Arc<Mutex<Stats>>,
) -> ProviderStream {
    let player = Player {
        steps: script.steps.into(),
        cancel,
        timeouts,
        stats,
        started_at: Instant::now(),
        seen_event: false,
        emitted_content: false,
        done: false,
    };
    Box::pin(futures_util::stream::unfold(player, |mut p| async move {
        p.next().await.map(|ev| (ev, p))
    }))
}
