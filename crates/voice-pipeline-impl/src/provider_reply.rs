//! `ProviderReply` — [`ReplySource`] na `ModelProvider` z własną historią rozmowy (append-only):
//! tura użytkownika → strumień modelu; tura asystentki trafia do historii **raz**, w pełnej
//! postaci (bloki myślenia nietknięte), a po przerwaniu z oznaczeniem usłyszanego prefiksu —
//! projekcja historii (`project_history` / `render_interrupted_turn`) dopisuje notkę dla modelu.
//! `app-core` podepnie w to miejsce prawdziwy czat sesji (gałęzie, zapis w `sessions`).

use std::collections::BTreeMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};

use futures_core::Stream;
use personas_contract::PersonaId;
use providers_contract::{
    CancellationToken, ChatRequest, Message, ModelProvider, PrivacyTag, ProviderEvent,
    ProviderStream, TurnAccumulator,
};
use voice_pipeline_contract::{ReplyChunk, ReplyOutcome, ReplyRequest, ReplySource, ReplyStream};

/// Prompt systemowy persony (np. `personas-contract::render_system_prompt` z obsadą sesji).
pub type PromptFn = Box<dyn Fn(&PersonaId) -> Option<String> + Send + Sync>;

#[derive(Default)]
struct History {
    messages: Vec<Message>,
    open: BTreeMap<u64, TurnAccumulator>,
}

/// Odpowiedzi z dostawcy modeli z historią append-only.
pub struct ProviderReply {
    provider: Arc<dyn ModelProvider>,
    model: String,
    prompt: PromptFn,
    privacy: PrivacyTag,
    history: Arc<Mutex<History>>,
}

impl std::fmt::Debug for ProviderReply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderReply")
            .field("model", &self.model)
            .field("messages", &self.history().len())
            .finish_non_exhaustive()
    }
}

fn lock(h: &Mutex<History>) -> MutexGuard<'_, History> {
    h.lock().unwrap_or_else(|p| p.into_inner())
}

impl ProviderReply {
    /// Dostawca + model; prompt systemowy per persona.
    pub fn new(
        provider: Arc<dyn ModelProvider>,
        model: impl Into<String>,
        prompt: PromptFn,
    ) -> Self {
        Self {
            provider,
            model: model.into(),
            prompt,
            privacy: PrivacyTag::Normal,
            history: Arc::default(),
        }
    }

    /// Tag prywatności sesji (sprawdza dostawca/Router).
    #[must_use]
    pub fn with_privacy(mut self, privacy: PrivacyTag) -> Self {
        self.privacy = privacy;
        self
    }

    /// Historia (zamknięte tury, w kolejności).
    pub fn history(&self) -> Vec<Message> {
        lock(&self.history).messages.clone()
    }

    fn close(h: &mut History, turn: u64, outcome: &ReplyOutcome) {
        let Some(acc) = h.open.remove(&turn) else {
            return;
        };
        let message = acc.finish().message;
        let message = match outcome {
            ReplyOutcome::Completed => message,
            ReplyOutcome::Interrupted { heard, approximate } => {
                message.with_interruption(heard.clone(), *approximate)
            }
        };
        h.messages.push(message);
    }
}

impl ReplySource for ProviderReply {
    fn start(&self, request: ReplyRequest, cancel: CancellationToken) -> ReplyStream {
        let chat = {
            let mut h = lock(&self.history);
            // Tury, których nikt nie zamknął (kontrakt tego wymaga) — zamknięte jako wysłuchane.
            let stale: Vec<u64> = h.open.keys().copied().collect();
            for turn in stale {
                Self::close(&mut h, turn, &ReplyOutcome::Completed);
            }
            h.messages.push(Message::user_text(request.text));
            h.open.insert(
                request.turn,
                TurnAccumulator::new(self.provider.id().clone()),
            );
            let mut chat = ChatRequest::new(self.model.clone(), h.messages.clone());
            chat.system = (self.prompt)(&request.persona);
            chat.meta.privacy.tag = self.privacy;
            chat
        };
        let inner = self.provider.stream(chat, cancel);
        Box::pin(ReplyAdapter {
            inner,
            history: Arc::clone(&self.history),
            turn: request.turn,
            finished: false,
        })
    }

    fn finish(&self, turn: u64, outcome: ReplyOutcome) {
        Self::close(&mut lock(&self.history), turn, &outcome);
    }
}

/// Strumień zdarzeń modelu → tekst; zdarzenia składane w turę (myślenie z podpisem zostaje).
struct ReplyAdapter {
    inner: ProviderStream,
    history: Arc<Mutex<History>>,
    turn: u64,
    finished: bool,
}

impl Stream for ReplyAdapter {
    type Item = ReplyChunk;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<ReplyChunk>> {
        loop {
            if self.finished {
                return Poll::Ready(None);
            }
            let event = match self.inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(event)) => event,
                Poll::Ready(None) => {
                    self.finished = true;
                    return Poll::Ready(Some(ReplyChunk::Done));
                }
                Poll::Pending => return Poll::Pending,
            };
            if let Some(acc) = lock(&self.history).open.get_mut(&self.turn) {
                acc.push(&event);
            }
            match event {
                ProviderEvent::TextDelta { text, .. } => {
                    return Poll::Ready(Some(ReplyChunk::Text(text)));
                }
                ProviderEvent::Stop { .. } => {
                    self.finished = true;
                    return Poll::Ready(Some(ReplyChunk::Done));
                }
                ProviderEvent::Error(e) => {
                    self.finished = true;
                    return Poll::Ready(Some(ReplyChunk::Failed(e.to_string())));
                }
                _ => {}
            }
        }
    }
}
