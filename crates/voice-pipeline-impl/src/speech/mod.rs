//! Mowa agentki: zadanie per wypowiedź automatu (fragmenty odpowiedzi, tekst wznowienia, mowa
//! proaktywna, filler) → synteza (`voice-tts`, głos agentki) → wyjście (`voice-audio`, tor głosu,
//! licznik próbek) → zdarzenia dla automatu (`TtsChunkQueued`, znaczniki słów).
//! Po `StopTts` wypowiedź nigdy nie gra dalej; to, czego nie zdążyła powiedzieć, czeka na wznowienie.

mod play;

use std::collections::VecDeque;
use std::task::{Context, Poll};

use personas_contract::PersonaId;
use voice_tts_contract::{
    CancelToken, SpeechStyle, TtsError, TtsRequest, TtsStream, split_sentences,
};

use crate::pipeline::{FILLER_BASE, Pipeline};
use crate::poll::BoxFut;

/// Synteza w toku.
pub(crate) enum Flight {
    Starting {
        text: String,
        fut: BoxFut<Result<TtsStream, TtsError>>,
    },
    Streaming {
        text: String,
        stream: TtsStream,
        received: usize,
    },
}

/// Zadanie mowy jednej wypowiedzi.
pub(crate) struct SpeechJob {
    pub persona: PersonaId,
    pub filler: bool,
    pub granted: bool,
    /// Odpowiedź, z której zadanie bierze kolejne fragmenty.
    pub turn: Option<u64>,
    pub reply_next: usize,
    /// Teksty spoza strumienia odpowiedzi (wznowienie, mowa proaktywna, filler).
    pub extra: VecDeque<String>,
    pub flight: Option<Flight>,
    pub cancel: CancelToken,
    pub ended: bool,
    pub dialog_chunks: usize,
}

/// Wypowiedź zatrzymana (`StopTts`) — materiał do wznowienia od punktu cięcia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parked {
    pub utterance: u64,
    pub persona: PersonaId,
    pub turn: Option<u64>,
    pub next_chunk: usize,
    pub inflight_rest: Option<String>,
    pub unsent: Vec<String>,
    pub base_words: usize,
    pub heard_words: usize,
}

impl SpeechJob {
    pub(crate) fn new(utterance: u64, persona: PersonaId) -> Self {
        Self {
            persona,
            filler: utterance >= FILLER_BASE,
            granted: false,
            turn: None,
            reply_next: 0,
            extra: VecDeque::new(),
            flight: None,
            cancel: CancelToken::new(),
            ended: false,
            dialog_chunks: 0,
        }
    }
}

impl Pipeline {
    /// `StartTts`: zadanie wypowiedzi przydzielone (głośnik jest nasz) — z odpowiedzią w toku.
    pub(crate) fn start_job(&mut self, utterance: u64, persona: PersonaId) {
        if let Some(job) = self.st.jobs.get_mut(&utterance) {
            job.granted = true;
            job.persona = persona;
            return;
        }
        let mut job = SpeechJob::new(utterance, persona);
        job.granted = true;
        if let Some(r) = self
            .st
            .reply
            .as_mut()
            .filter(|r| r.utterance.is_none() && !r.bound_once)
        {
            r.utterance = Some(utterance);
            r.bound_once = true;
            job.turn = Some(r.turn);
        }
        self.st.jobs.insert(utterance, job);
    }

    /// Mowa proaktywna / filler: zadanie z gotowym tekstem.
    pub(crate) fn text_job(&mut self, utterance: u64, persona: PersonaId, text: &str) {
        let mut job = SpeechJob::new(utterance, persona);
        job.granted = true;
        job.extra.push_back(self.persona.normalize_pl(text));
        self.st.jobs.insert(utterance, job);
    }

    /// `ResumeFrom`: nowa wypowiedź = znany tekst od punktu cięcia + reszta przerwanej.
    pub(crate) fn resume_job(&mut self, from: u64, offset: usize, text: String, utterance: u64) {
        let parked = self.st.parked.take().filter(|p| p.utterance == from);
        let persona = parked
            .as_ref()
            .map_or_else(|| self.st.active_persona.clone(), |p| p.persona.clone());
        let mut job = SpeechJob::new(utterance, persona);
        job.extra.push_back(text);
        if let Some(p) = parked {
            job.extra.extend(p.inflight_rest);
            let base = if offset == 0 {
                p.base_words
            } else {
                p.base_words + p.heard_words
            };
            match self.st.reply.as_mut().filter(|r| Some(r.turn) == p.turn) {
                Some(r) => {
                    r.utterance = Some(utterance);
                    r.word_base = base;
                    r.pending_heard = None;
                    job.turn = Some(r.turn);
                    job.reply_next = p.next_chunk;
                }
                None => job.extra.extend(p.unsent),
            }
        }
        self.st.jobs.insert(utterance, job);
    }

    /// `StopTts` / `StopFiller`: anulowanie syntezy, cisza na wyjściu (≤ 20 ms), materiał na bok.
    pub(crate) fn stop_job(&mut self, utterance: u64) {
        if let Some(mut job) = self.st.jobs.remove(&utterance) {
            job.cancel.cancel();
            self.tts.stop(utterance);
            let inflight_rest = match job.flight.take() {
                Some(Flight::Starting { text, .. }) => Some(text),
                Some(Flight::Streaming { text, received, .. }) => {
                    let rest = split_sentences(&text)
                        .into_iter()
                        .skip(received)
                        .collect::<Vec<_>>()
                        .join(" ");
                    (!rest.is_empty()).then_some(rest)
                }
                None => None,
            };
            if !job.filler {
                let base_words = self
                    .st
                    .reply
                    .as_ref()
                    .filter(|r| r.utterance == Some(utterance))
                    .map_or(0, |r| r.word_base);
                self.st.parked = Some(Parked {
                    utterance,
                    persona: job.persona,
                    turn: job.turn,
                    next_chunk: job.reply_next,
                    inflight_rest,
                    unsent: job.extra.into_iter().collect(),
                    base_words,
                    heard_words: 0,
                });
            }
        }
        if let Err(e) = self.output.stop_all() {
            self.degraded("audio", e.to_string());
        }
    }

    /// `ClearSpeechQueue`: porzuca kolejkowane teksty zadań, które jeszcze nie mówią.
    pub(crate) fn clear_speech_queue(&mut self) {
        self.st.jobs.retain(|_, j| j.granted);
    }

    /// Filler (poza prefiksem, przerywalny) — tylko gdy głośnika nie trzyma inna agentka.
    pub(crate) fn play_filler(&mut self) {
        let persona = self.st.active_persona.clone();
        let holder = voice_dialog_contract::SpeakerLock::holder(self.speaker.as_ref());
        if holder.is_some_and(|h| h.persona != persona) {
            return;
        }
        let id = self.st.next_filler;
        self.st.next_filler += 1;
        let text = self.cfg.filler_text.clone();
        self.text_job(id, persona, &text);
    }

    pub(crate) fn stop_fillers(&mut self) {
        let fillers: Vec<u64> = self
            .st
            .jobs
            .keys()
            .copied()
            .filter(|u| *u >= FILLER_BASE)
            .collect();
        for u in fillers {
            self.stop_job(u);
        }
    }

    /// Prowadzi wszystkie przydzielone zadania mowy (bez czekania).
    pub(crate) fn poll_jobs(&mut self, cx: &mut Context<'_>) {
        let ids: Vec<u64> = self.st.jobs.keys().copied().collect();
        for id in ids {
            self.drive_job(id, cx);
        }
    }

    fn next_text(&mut self, id: u64) -> Option<String> {
        let reply = self.st.reply.as_ref();
        let job = self.st.jobs.get_mut(&id)?;
        if let Some(t) = job.extra.pop_front() {
            return Some(t);
        }
        let r = reply.filter(|r| Some(r.turn) == job.turn)?;
        let (_, spoken) = r.chunks.get(job.reply_next)?;
        job.reply_next += 1;
        Some(spoken.clone())
    }

    /// Audio wypowiedzi w kolejce wyjścia, które jeszcze nie zagrało (ms).
    fn ahead_ms(&mut self, id: u64) -> u64 {
        self.output.position(id).map_or(0, |p| {
            let backlog = p.queued_samples.saturating_sub(p.rendered_samples);
            backlog * 1_000 / u64::from(p.sample_rate.max(1))
        })
    }

    fn job_closed(&self, id: u64) -> bool {
        let Some(job) = self.st.jobs.get(&id) else {
            return true;
        };
        match (job.turn, self.st.reply.as_ref()) {
            (Some(t), Some(r)) if r.turn == t => r.done && job.reply_next >= r.chunks.len(),
            _ => true,
        }
    }

    fn drive_job(&mut self, id: u64, cx: &mut Context<'_>) {
        for _ in 0..64 {
            let Some(job) = self.st.jobs.get_mut(&id) else {
                return;
            };
            if !job.granted {
                return;
            }
            match job.flight.take() {
                None => {
                    if self.ahead_ms(id) > u64::from(self.cfg.speak_ahead_ms) {
                        return;
                    }
                    if let Some(text) = self.next_text(id) {
                        self.start_synth(id, text);
                        continue;
                    }
                    if self.job_closed(id) {
                        self.end_job(id);
                    }
                    return;
                }
                Some(Flight::Starting { text, mut fut }) => match fut.as_mut().poll(cx) {
                    Poll::Ready(Ok(stream)) => {
                        job.flight = Some(Flight::Streaming {
                            text,
                            stream,
                            received: 0,
                        });
                    }
                    Poll::Ready(Err(e)) => self.degraded("tts", e.to_string()),
                    Poll::Pending => {
                        job.flight = Some(Flight::Starting { text, fut });
                        return;
                    }
                },
                Some(Flight::Streaming {
                    text,
                    mut stream,
                    received,
                }) => match stream.poll_recv(cx) {
                    Poll::Ready(Some(Ok(chunk))) => {
                        if !chunk.is_last {
                            job.flight = Some(Flight::Streaming {
                                text: text.clone(),
                                stream,
                                received: received + 1,
                            });
                        }
                        self.play_chunk(id, &text, &chunk);
                    }
                    Poll::Ready(Some(Err(e))) => self.degraded("tts", e.to_string()),
                    Poll::Ready(None) => {}
                    Poll::Pending => {
                        job.flight = Some(Flight::Streaming {
                            text,
                            stream,
                            received,
                        });
                        return;
                    }
                },
            }
        }
    }

    fn start_synth(&mut self, id: u64, text: String) {
        let Some(job) = self.st.jobs.get_mut(&id) else {
            return;
        };
        let request = TtsRequest {
            utterance: id,
            persona: job.persona.clone(),
            text: text.clone(),
            style: SpeechStyle::default(),
            cacheable: job.filler,
            privacy: self.cfg.privacy,
        };
        let tts = std::sync::Arc::clone(&self.tts);
        let cancel = job.cancel.clone();
        job.flight = Some(Flight::Starting {
            text,
            fut: Box::pin(async move { tts.synth(request, cancel).await }),
        });
    }

    fn end_job(&mut self, id: u64) {
        let Some(job) = self.st.jobs.get_mut(&id) else {
            return;
        };
        if job.ended {
            return;
        }
        job.ended = true;
        if let Err(e) = self.output.end_utterance(id) {
            self.degraded("audio", e.to_string());
        }
    }
}
