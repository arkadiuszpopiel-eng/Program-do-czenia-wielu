//! Łącze STT: operacje (start, ramki, partial na żądanie, koniec, anulowanie) wykonywane po kolei
//! jedną przyszłością naraz (kolejność per wypowiedź), wyniki odbierane w kolejnych krokach.

use std::collections::VecDeque;
use std::sync::Arc;
use std::task::Context;

use voice_audio_contract::Frame;
use voice_stt_contract::{Stt, SttError, Transcript, UtteranceId};

use crate::poll::{BoxFut, poll_slot};

/// Operacja STT.
#[derive(Debug, Clone)]
pub(crate) enum SttOp {
    Start(UtteranceId),
    Push(UtteranceId, Vec<Frame>),
    Partial(UtteranceId),
    End(UtteranceId),
    Cancel(UtteranceId),
}

/// Wynik operacji STT.
#[derive(Debug, Clone)]
pub(crate) enum SttOut {
    /// Partial (z rytmu dwóch przebiegów albo na żądanie).
    Partial(UtteranceId, Transcript),
    /// Final (albo błąd zamknięcia — potok ponawia z buforem wypowiedzi).
    Final(UtteranceId, Result<Transcript, SttError>),
    /// Błąd startu/ramki.
    Failed(UtteranceId, SttError),
}

pub(crate) struct SttLink {
    stt: Arc<dyn Stt>,
    ops: VecDeque<SttOp>,
    in_flight: Option<BoxFut<Vec<SttOut>>>,
    partial_in_flight: bool,
}

fn run(stt: Arc<dyn Stt>, op: SttOp) -> BoxFut<Vec<SttOut>> {
    Box::pin(async move {
        let mut out = Vec::new();
        match op {
            SttOp::Start(id) => {
                if let Err(e) = stt.start_utterance(id).await {
                    out.push(SttOut::Failed(id, e));
                }
            }
            SttOp::Push(id, frames) => {
                for f in &frames {
                    match stt.push(id, f).await {
                        Ok(Some(t)) => out.push(SttOut::Partial(id, t)),
                        Ok(None) => {}
                        Err(e) => {
                            out.push(SttOut::Failed(id, e));
                            break;
                        }
                    }
                }
            }
            SttOp::Partial(id) => match stt.partial_now(id).await {
                Ok(Some(t)) => out.push(SttOut::Partial(id, t)),
                Ok(None) => {}
                Err(e) => out.push(SttOut::Failed(id, e)),
            },
            SttOp::End(id) => out.push(SttOut::Final(id, stt.end_utterance(id).await)),
            SttOp::Cancel(id) => stt.cancel(id).await,
        }
        out
    })
}

impl SttLink {
    pub(crate) fn new(stt: Arc<dyn Stt>) -> Self {
        Self {
            stt,
            ops: VecDeque::new(),
            in_flight: None,
            partial_in_flight: false,
        }
    }

    pub(crate) fn submit(&mut self, op: SttOp) {
        self.ops.push_back(op);
    }

    /// Czy partial na żądanie czeka albo trwa (nie dokładamy kolejnego).
    pub(crate) fn partial_pending(&self) -> bool {
        self.partial_in_flight || self.ops.iter().any(|o| matches!(o, SttOp::Partial(_)))
    }

    /// Wykonuje operacje, dopóki wyniki są gotowe bez czekania.
    pub(crate) fn poll(&mut self, cx: &mut Context<'_>) -> Vec<SttOut> {
        let mut results = Vec::new();
        loop {
            if self.in_flight.is_some() {
                match poll_slot(&mut self.in_flight, cx) {
                    Some(out) => {
                        self.partial_in_flight = false;
                        results.extend(out);
                    }
                    None => break,
                }
                continue;
            }
            let Some(op) = self.ops.pop_front() else {
                break;
            };
            self.partial_in_flight = matches!(op, SttOp::Partial(_));
            self.in_flight = Some(run(Arc::clone(&self.stt), op));
        }
        results
    }

    /// Zdarzenia silnika (`voice.stt.*`).
    pub(crate) fn take_events(&self) -> Vec<voice_stt_contract::SttEvent> {
        self.stt.take_events()
    }
}
