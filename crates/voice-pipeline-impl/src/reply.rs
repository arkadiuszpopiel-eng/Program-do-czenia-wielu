//! Odpowiedź agentki: strumień z [`ReplySource`] → chunker i normalizacja PL (`voice-persona`) →
//! fragmenty dla syntezy. Wynik tury trafia do historii raz (append-only): wysłuchana albo
//! przerwana z usłyszanym prefiksem przeliczonym na tekst oryginału.

use std::task::{Context, Poll};

use personas_contract::PersonaId;
use providers_contract::CancellationToken;
use voice_dialog_contract::{
    ApproxTrim, DialogEvent, HeardPrefix, InterruptIntent, TurnSource, UtteranceId, heard_prefix,
};
use voice_persona_contract::SpeechChunker;
use voice_pipeline_contract::{
    PipelineEvent, ReplyChunk, ReplyOutcome, ReplyRequest, ReplyStream, TurnLatency,
};

use crate::pipeline::Pipeline;

/// Odpowiedź w toku.
pub(crate) struct ActiveReply {
    pub turn: u64,
    pub persona: PersonaId,
    pub stream: Option<ReplyStream>,
    pub cancel: CancellationToken,
    pub chunker: Box<dyn SpeechChunker>,
    /// Fragmenty (tekst oryginału, tekst mówiony po normalizacji).
    pub chunks: Vec<(String, String)>,
    pub done: bool,
    pub had_text: bool,
    pub ready_sent: bool,
    pub bound_once: bool,
    /// Wypowiedź automatu, która ją teraz mówi.
    pub utterance: Option<u64>,
    /// Słowa mówione usłyszane przed bieżącą wypowiedzią (wznowienie od punktu cięcia).
    pub word_base: usize,
    /// Prefiks z przerwania „czekaj/pauza” (generowanie trwa, tura jeszcze otwarta).
    pub pending_heard: Option<(String, bool)>,
}

/// Prefiks w tekście oryginału: `words` słów mówionych przeliczone fragment po fragmencie
/// (cały fragment albo proporcjonalna część jego słów).
pub fn raw_prefix(chunks: &[(String, String)], words: usize) -> String {
    let mut left = words;
    let mut out: Vec<&str> = Vec::new();
    for (raw, spoken) in chunks {
        if left == 0 {
            break;
        }
        let raw_words: Vec<&str> = raw.split_whitespace().collect();
        let spoken_n = spoken.split_whitespace().count();
        if left >= spoken_n {
            out.extend(&raw_words);
            left -= spoken_n;
        } else {
            let take = raw_words.len() * left / spoken_n.max(1);
            out.extend(raw_words.iter().take(take));
            left = 0;
        }
    }
    out.join(" ")
}

impl Pipeline {
    /// `SubmitTurn`: nowa odpowiedź (poprzednia — jeśli jeszcze otwarta — zamykana jako przerwana).
    pub(crate) fn submit_reply(
        &mut self,
        turn: u64,
        text: String,
        intent: Option<InterruptIntent>,
        source: TurnSource,
    ) {
        if self.st.reply.is_some() {
            self.cancel_generation(None);
        }
        self.st.parked = None;
        let persona = self.st.active_persona.clone();
        let cancel = CancellationToken::new();
        let voice = if source == TurnSource::Voice {
            self.take_voice_provenance(turn)
        } else {
            None
        };
        let stream = self.replies.start(
            ReplyRequest {
                turn,
                persona: persona.clone(),
                text,
                source,
                intent,
                voice,
            },
            cancel.clone(),
        );
        self.st.reply = Some(ActiveReply {
            turn,
            persona,
            stream: Some(stream),
            cancel,
            chunker: self.persona.chunker(self.cfg.chunker),
            chunks: Vec::new(),
            done: false,
            had_text: false,
            ready_sent: false,
            bound_once: false,
            utterance: None,
            word_base: 0,
            pending_heard: None,
        });
        self.st.turns += 1;
        let mut latency = self.st.pending_latency.take().unwrap_or_default();
        latency.turn = turn;
        self.st.latency = latency;
        self.st.latency_published = false;
    }

    /// Odbiera tekst odpowiedzi bez czekania.
    pub(crate) fn poll_reply(&mut self, cx: &mut Context<'_>) {
        let Some(r) = self.st.reply.as_mut() else {
            return;
        };
        let Some(stream) = r.stream.as_mut() else {
            return;
        };
        let mut texts = Vec::new();
        let mut end: Option<Option<String>> = None;
        loop {
            match stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(ReplyChunk::Text(t))) => texts.push(t),
                Poll::Ready(Some(ReplyChunk::Done) | None) => {
                    end = Some(None);
                    break;
                }
                Poll::Ready(Some(ReplyChunk::Failed(reason))) => {
                    end = Some(Some(reason));
                    break;
                }
                Poll::Pending => break,
            }
        }
        let mut raw = Vec::new();
        for t in &texts {
            r.had_text = true;
            raw.extend(r.chunker.push(t).into_iter().map(|c| c.text));
        }
        let mut failure = None;
        if let Some(reason) = end {
            r.stream = None;
            raw.extend(r.chunker.finish().into_iter().map(|c| c.text));
            failure = reason;
            if !r.had_text {
                // Pusta albo nieudana odpowiedź — agentka mówi krótki komunikat zamiast milczeć.
                r.had_text = true;
                raw.push(self.cfg.failure_text.clone());
            }
            r.done = true;
        }
        let ready = (r.had_text && !r.ready_sent).then(|| r.persona.clone());
        r.ready_sent |= ready.is_some();
        for text in raw {
            let spoken = self.persona.normalize_pl(&text);
            if !spoken.trim().is_empty() {
                r.chunks.push((text, spoken));
            }
        }
        if let Some(reason) = failure {
            self.degraded("reply", reason);
        }
        if let Some(persona) = ready {
            if self.st.latency.first_text_ms.is_none() {
                self.st.latency.first_text_ms = Some(self.st.now_ms);
            }
            self.dialog_event(DialogEvent::ResponseReady { persona });
        }
    }

    /// Powiadomienie o przerwaniu (twardy stop): prefiks do historii, stan UI, zdarzenie.
    pub(crate) fn on_interrupted(&mut self, heard: &HeardPrefix) {
        self.st.interruptions += 1;
        self.st.heard = Some(heard.clone());
        if let Some(p) = self
            .st
            .parked
            .as_mut()
            .filter(|p| p.utterance == heard.utterance.0)
        {
            p.heard_words = heard.words;
        }
        let mut turn = None;
        let mut raw = heard.text.clone();
        if let Some(r) = self
            .st
            .reply
            .as_mut()
            .filter(|r| r.utterance == Some(heard.utterance.0) || r.utterance.is_none())
        {
            raw = raw_prefix(&r.chunks, r.word_base + heard.words);
            r.pending_heard = Some((raw.clone(), heard.approximate));
            r.utterance = None;
            turn = Some(r.turn);
        }
        self.publish(&PipelineEvent::HeardPrefix {
            turn,
            heard: heard.clone(),
            heard_raw: raw,
        });
    }

    /// Anulowanie generowania (barge-in, „stop”, `Esc`): tura zamykana jako przerwana.
    pub(crate) fn cancel_generation(&mut self, heard: Option<&HeardPrefix>) {
        let Some(r) = self.st.reply.as_ref() else {
            return;
        };
        let outcome = if let Some((heard, approximate)) = r.pending_heard.clone() {
            ReplyOutcome::Interrupted { heard, approximate }
        } else if let Some(h) = heard {
            ReplyOutcome::Interrupted {
                heard: raw_prefix(&r.chunks, r.word_base + h.words),
                approximate: h.approximate,
            }
        } else {
            let state = self.dialog.state();
            match state
                .utterance
                .as_ref()
                .filter(|u| Some(u.id) == r.utterance.map(UtteranceId))
            {
                Some(u) => {
                    let h = heard_prefix(u, ApproxTrim::Word);
                    ReplyOutcome::Interrupted {
                        heard: raw_prefix(&r.chunks, r.word_base + h.words),
                        approximate: h.approximate,
                    }
                }
                None => ReplyOutcome::Interrupted {
                    heard: String::new(),
                    approximate: false,
                },
            }
        };
        self.finalize_reply(outcome);
    }

    /// Zamyka turę w historii i porzuca strumień (fragmenty niewypowiedziane zostają do wznowienia).
    pub(crate) fn finalize_reply(&mut self, outcome: ReplyOutcome) {
        let Some(r) = self.st.reply.take() else {
            return;
        };
        r.cancel.cancel();
        if let Some(p) = self.st.parked.as_mut().filter(|p| p.turn == Some(r.turn)) {
            p.unsent.extend(
                r.chunks
                    .iter()
                    .skip(p.next_chunk)
                    .map(|(_, spoken)| spoken.clone()),
            );
            p.turn = None;
        }
        self.replies.finish(r.turn, outcome);
        self.wake_processing(false);
    }

    /// Rejestruje pierwsze audio odpowiedzi (licznik próbek wyjścia − już wybrzmiały czas).
    pub(crate) fn note_first_audio(&mut self, utterance: u64) {
        let is_reply = self
            .st
            .jobs
            .get(&utterance)
            .is_some_and(|j| j.turn == Some(self.st.latency.turn) && j.turn.is_some());
        if !is_reply || self.st.latency.first_audio_ms.is_some() {
            return;
        }
        if let Some(pos) = self.output.position(utterance) {
            if pos.rendered_samples == 0 || pos.sample_rate == 0 {
                return;
            }
            let rendered_ms = pos.rendered_samples * 1_000 / u64::from(pos.sample_rate);
            let latency_ms = u64::try_from(pos.output_latency.as_millis()).unwrap_or(0);
            self.st.latency.first_audio_ms =
                Some(self.st.now_ms.saturating_sub(rendered_ms) + latency_ms);
        }
    }

    /// Publikuje opóźnienia tury, gdy znane jest pierwsze audio.
    pub(crate) fn publish_latency(&mut self) {
        if self.st.latency_published || self.st.latency.first_audio_ms.is_none() {
            return;
        }
        self.st.latency_published = true;
        let l: TurnLatency = self.st.latency;
        self.publish(&PipelineEvent::Latency(l));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_prefix_maps_spoken_words_to_original() {
        let chunks = vec![
            ("Mam 3 pliki.".to_owned(), "Mam trzy pliki.".to_owned()),
            (
                "Spotkanie o 10:30 jutro.".to_owned(),
                "Spotkanie o dziesiątej trzydzieści jutro.".to_owned(),
            ),
        ];
        assert_eq!(raw_prefix(&chunks, 0), "");
        assert_eq!(raw_prefix(&chunks, 3), "Mam 3 pliki.");
        assert_eq!(raw_prefix(&chunks, 5), "Mam 3 pliki. Spotkanie");
        assert_eq!(
            raw_prefix(&chunks, 99),
            "Mam 3 pliki. Spotkanie o 10:30 jutro."
        );
    }
}
