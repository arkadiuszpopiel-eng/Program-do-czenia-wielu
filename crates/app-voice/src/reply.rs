//! `ReplySource` na czacie sesji: tura użytkownika z mowy idzie do tej samej sesji co tekst
//! (historia append-only), odpowiedź strumieniuje się do potoku i do UI jednocześnie, a wynik
//! tury (przerwana → usłyszany prefiks) trafia do historii po zamknięciu przez automat.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll};

use app_api::ports::{VoiceChat, VoiceChunk, VoiceTurnRef};
use futures_core::Stream;
use providers_contract::CancellationToken;
use tokio::sync::mpsc;
use voice_pipeline_contract::{ReplyChunk, ReplyOutcome, ReplyRequest, ReplySource, ReplyStream};

/// Stan tury potoku (numer automatu → tura czatu).
enum Slot {
    /// Tura czatu już istnieje.
    Started(VoiceTurnRef),
    /// Automat zamknął turę, zanim czat ją utworzył (np. natychmiastowy barge-in).
    Finished(Option<(String, bool)>),
}

/// Odpowiedzi rozmowy głosowej z czatu sesji.
pub struct ChatReply {
    chat: Arc<dyn VoiceChat>,
    slots: Arc<Mutex<HashMap<u64, Slot>>>,
}

fn lock(m: &Mutex<HashMap<u64, Slot>>) -> MutexGuard<'_, HashMap<u64, Slot>> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn heard_of(outcome: &ReplyOutcome) -> Option<(String, bool)> {
    match outcome {
        ReplyOutcome::Completed => None,
        ReplyOutcome::Interrupted { heard, approximate } => Some((heard.clone(), *approximate)),
    }
}

impl ChatReply {
    /// Źródło odpowiedzi na czacie.
    pub fn new(chat: Arc<dyn VoiceChat>) -> Self {
        Self {
            chat,
            slots: Arc::default(),
        }
    }
}

impl ReplySource for ChatReply {
    fn start(&self, request: ReplyRequest, cancel: CancellationToken) -> ReplyStream {
        let (tx, rx) = mpsc::unbounded_channel();
        let chat = self.chat.clone();
        let slots = self.slots.clone();
        tokio::spawn(async move {
            let started = chat
                .voice_turn(request.persona.as_str(), &request.text, cancel)
                .await;
            let mut turn = match started {
                Ok(turn) => turn,
                Err(e) => {
                    let _ = tx.send(ReplyChunk::Failed(e.message));
                    return;
                }
            };
            let early = {
                let mut slots = lock(&slots);
                match slots.remove(&request.turn) {
                    Some(Slot::Finished(heard)) => Some(heard),
                    _ => {
                        slots.insert(request.turn, Slot::Started(turn.turn.clone()));
                        None
                    }
                }
            };
            if let Some(heard) = early {
                chat.voice_finish(turn.turn.clone(), heard).await;
            }
            while let Some(chunk) = turn.chunks.recv().await {
                let (chunk, last) = match chunk {
                    VoiceChunk::Text(t) => (ReplyChunk::Text(t), false),
                    VoiceChunk::Done => (ReplyChunk::Done, true),
                    VoiceChunk::Failed(e) => (ReplyChunk::Failed(e), true),
                };
                if tx.send(chunk).is_err() || last {
                    break;
                }
            }
        });
        Box::pin(ChunkStream { rx })
    }

    fn finish(&self, turn: u64, outcome: ReplyOutcome) {
        let heard = heard_of(&outcome);
        let started = {
            let mut slots = lock(&self.slots);
            match slots.remove(&turn) {
                Some(Slot::Started(r)) => Some(r),
                _ => {
                    slots.insert(turn, Slot::Finished(heard.clone()));
                    None
                }
            }
        };
        if let Some(r) = started {
            let chat = self.chat.clone();
            tokio::spawn(async move { chat.voice_finish(r, heard).await });
        }
    }
}

/// Strumień fragmentów z kanału.
struct ChunkStream {
    rx: mpsc::UnboundedReceiver<ReplyChunk>,
}

impl Stream for ChunkStream {
    type Item = ReplyChunk;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<ReplyChunk>> {
        self.rx.poll_recv(cx)
    }
}
