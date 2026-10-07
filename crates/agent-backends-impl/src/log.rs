//! Dziennik zdarzeń zadania: kolejne koperty z `seq` bez luk, jedno zdarzenie końcowe,
//! strumienie odtwarzające od początku i potem na żywo.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use agent_backends_contract::{AgentEvent, AgentEventEnvelope, AgentEventStream, TaskId};
use tokio::sync::{mpsc, watch};

struct Inner {
    events: Vec<AgentEventEnvelope>,
    finished: bool,
}

/// Dziennik zadania.
pub struct TaskLog {
    task: TaskId,
    started: Instant,
    inner: Mutex<Inner>,
    tick: watch::Sender<usize>,
    forward: Option<mpsc::UnboundedSender<AgentEventEnvelope>>,
}

impl TaskLog {
    /// Nowy dziennik; `forward` — opcjonalna kopia zdarzeń (np. na magistralę).
    pub fn new(
        task: TaskId,
        forward: Option<mpsc::UnboundedSender<AgentEventEnvelope>>,
    ) -> Arc<Self> {
        let (tick, _) = watch::channel(0);
        Arc::new(Self {
            task,
            started: Instant::now(),
            inner: Mutex::new(Inner {
                events: Vec::new(),
                finished: false,
            }),
            tick,
            forward,
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Zadanie.
    pub fn task(&self) -> &TaskId {
        &self.task
    }

    /// Dopisuje zdarzenie; po zdarzeniu końcowym kolejne są ignorowane (zwraca `false`).
    pub fn emit(&self, event: AgentEvent) -> bool {
        let envelope = {
            let mut inner = self.lock();
            if inner.finished {
                return false;
            }
            let at_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
            let seq = inner.events.len() as u64;
            inner.finished = event.is_terminal();
            let envelope = AgentEventEnvelope::new(self.task.clone(), seq, at_ms, event);
            inner.events.push(envelope.clone());
            envelope
        };
        if let Some(tx) = &self.forward {
            let _ = tx.send(envelope);
        }
        let len = self.lock().events.len();
        self.tick.send_replace(len);
        true
    }

    /// Czy zadanie ma już zdarzenie końcowe.
    pub fn is_finished(&self) -> bool {
        self.lock().finished
    }

    /// Kopia zdarzeń.
    pub fn snapshot(&self) -> Vec<AgentEventEnvelope> {
        self.lock().events.clone()
    }

    /// Strumień: od początku, potem na żywo, koniec po zdarzeniu końcowym.
    pub fn stream(self: &Arc<Self>) -> AgentEventStream {
        let rx = self.tick.subscribe();
        let state = (self.clone(), rx, 0usize);
        Box::pin(futures_util::stream::unfold(
            state,
            |(log, mut rx, idx)| async move {
                loop {
                    let (next, finished) = {
                        let inner = log.lock();
                        (inner.events.get(idx).cloned(), inner.finished)
                    };
                    if let Some(ev) = next {
                        return Some((ev, (log, rx, idx + 1)));
                    }
                    if finished {
                        return None;
                    }
                    if rx.changed().await.is_err() {
                        return None;
                    }
                }
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_backends_contract::BackendError;
    use futures_util::StreamExt;

    #[tokio::test]
    async fn replay_then_live_and_single_terminal() {
        let log = TaskLog::new(TaskId("t".into()), None);
        assert!(log.emit(AgentEvent::ColdStart { ms: 1 }));
        let mut s = log.stream();
        assert_eq!(s.next().await.unwrap().seq, 0);
        let l2 = log.clone();
        tokio::spawn(async move {
            l2.emit(AgentEvent::Step { text: "a".into() });
            l2.emit(AgentEvent::Error {
                error: BackendError::Cancelled,
            });
            assert!(!l2.emit(AgentEvent::Step {
                text: "po końcu".into()
            }));
        });
        assert_eq!(s.next().await.unwrap().seq, 1);
        assert!(s.next().await.unwrap().event.is_terminal());
        assert!(s.next().await.is_none());
        assert!(log.is_finished());
        assert_eq!(log.snapshot().len(), 3);
        assert_eq!(log.stream().count().await, 3);
    }
}
