//! Odtwarzanie fragmentu syntezy: wyjście (tor głosu agentki), dziennik, zdarzenia automatu
//! (`TtsChunkQueued`, znaczniki słów względem fragmentu) i koniec wypowiedzi.

use voice_audio_contract::SourceId;
use voice_dialog_contract::{DialogEvent, MarkSource, SpeakerLock, UtteranceId, WordMark};
use voice_pipeline_contract::ReplyOutcome;
use voice_tts_contract::{MarksKind, TtsChunk, split_sentences};

use crate::pipeline::{Pipeline, PlayRecord};

/// Tekst fragmentu TTS: słowa ze znaczników (te same słowa co w żądaniu) albo zdanie `seq`.
fn chunk_text(request: &str, chunk: &TtsChunk) -> String {
    if !chunk.marks.is_empty() {
        return chunk
            .marks
            .iter()
            .map(|m| m.word.as_str())
            .collect::<Vec<_>>()
            .join(" ");
    }
    let sentences = split_sentences(request);
    if sentences.len() <= 1 {
        return request.trim().to_owned();
    }
    sentences
        .get(chunk.seq as usize)
        .cloned()
        .unwrap_or_else(|| request.trim().to_owned())
}

/// Znaczniki słów fragmentu dla automatu (znaki w tekście fragmentu, czas w audio fragmentu).
fn dialog_marks(text: &str, chunk: &TtsChunk) -> Option<(Vec<WordMark>, MarkSource)> {
    let source = match chunk.marks_kind {
        MarksKind::Native => MarkSource::Tts,
        MarksKind::ForcedAlign => MarkSource::Alignment,
        MarksKind::Estimated => return None,
    };
    let offset = chunk.audio.ts.as_ms();
    let mut pos = 0usize;
    let mut words = Vec::new();
    for w in text.split(' ') {
        let len = w.chars().count();
        if len > 0 {
            words.push((pos, pos + len));
        }
        pos += len + 1;
    }
    if words.len() != chunk.marks.len() {
        return None;
    }
    let marks = words
        .into_iter()
        .zip(&chunk.marks)
        .map(|((s, e), m)| WordMark {
            char_start: s,
            char_end: e,
            start_ms: u64::from(m.start_ms).saturating_sub(offset),
            end_ms: u64::from(m.end_ms).saturating_sub(offset),
        })
        .collect();
    Some((marks, source))
}

impl Pipeline {
    /// Fragment syntezy → wyjście + automat (tekst fragmentu, znaczniki słów).
    pub(super) fn play_chunk(&mut self, id: u64, request: &str, chunk: &TtsChunk) {
        let Some(job) = self.st.jobs.get_mut(&id) else {
            return;
        };
        let source = if job.filler {
            SourceId::Filler(job.persona.clone())
        } else {
            SourceId::Tts(job.persona.clone())
        };
        let (filler, index, persona) = (job.filler, job.dialog_chunks, job.persona.clone());
        job.dialog_chunks += 1;
        if let Err(e) = self.output.play(&source, id, &chunk.audio) {
            self.degraded("audio", e.to_string());
        }
        if self.cfg.trace_capacity > 0 {
            if self.st.plays.len() >= self.cfg.trace_capacity {
                self.st.plays.pop_front();
            }
            let holder = SpeakerLock::holder(self.speaker.as_ref());
            self.st.seq += 1;
            self.st.plays.push_back(PlayRecord {
                at_ms: self.st.now_ms,
                seq: self.st.seq,
                utterance: id,
                persona,
                filler,
                speaker_holder: holder,
            });
        }
        if filler {
            return;
        }
        if self.st.latency.first_chunk_ms.is_none() {
            self.st.latency.first_chunk_ms = Some(self.st.now_ms);
        }
        let text = chunk_text(request, chunk);
        let audio_ms = u64::try_from(chunk.audio.duration().as_millis()).unwrap_or(0);
        let marks = dialog_marks(&text, chunk);
        self.dialog_event(DialogEvent::TtsChunkQueued {
            utterance: UtteranceId(id),
            text,
            audio_ms,
        });
        if let Some((marks, source)) = marks {
            self.dialog_event(DialogEvent::TtsWordMarks {
                utterance: UtteranceId(id),
                chunk: index,
                marks,
                source,
            });
        }
    }

    /// Koniec odtwarzania wypowiedzi (raport miksera).
    pub(crate) fn on_playback_finished(&mut self, utterance: u64, stopped: bool) {
        if stopped || !self.st.jobs.get(&utterance).is_some_and(|j| j.ended) {
            return;
        }
        let Some(job) = self.st.jobs.remove(&utterance) else {
            return;
        };
        if job.filler {
            return;
        }
        let complete = job.turn.is_some_and(|t| {
            self.st
                .reply
                .as_ref()
                .is_some_and(|r| r.turn == t && r.done)
        });
        if complete {
            self.finalize_reply(ReplyOutcome::Completed);
        }
        self.dialog_event(DialogEvent::ResponseFinished {
            utterance: UtteranceId(utterance),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_audio_contract::{Frame, MediaTime};

    fn chunk(marks: Vec<voice_tts_contract::WordMark>, kind: MarksKind, offset: u64) -> TtsChunk {
        TtsChunk {
            utterance: 1,
            seq: 1,
            audio: Frame::mono(vec![0.0; 2_400], 24_000, MediaTime::from_ms(offset)),
            marks,
            marks_kind: kind,
            is_last: true,
            engine: "x".into(),
        }
    }

    fn mark(i: u32, w: &str, s: u32, e: u32) -> voice_tts_contract::WordMark {
        voice_tts_contract::WordMark {
            word_idx: i,
            word: w.into(),
            start_ms: s,
            end_ms: e,
        }
    }

    #[test]
    fn chunk_text_and_marks_are_relative_to_fragment() {
        let c = chunk(
            vec![mark(2, "Kot", 1_000, 1_200), mark(3, "śpi.", 1_250, 1_500)],
            MarksKind::Native,
            1_000,
        );
        let text = chunk_text("Ala ma. Kot śpi.", &c);
        assert_eq!(text, "Kot śpi.");
        let (marks, src) = dialog_marks(&text, &c).unwrap();
        assert_eq!(src, MarkSource::Tts);
        assert_eq!((marks[1].char_start, marks[1].char_end), (4, 8));
        assert_eq!((marks[0].start_ms, marks[1].end_ms), (0, 500));
        let est = chunk(vec![mark(0, "a", 0, 1)], MarksKind::Estimated, 0);
        assert!(dialog_marks("a", &est).is_none());
        let aligned = chunk(vec![mark(0, "a", 0, 1)], MarksKind::ForcedAlign, 0);
        assert_eq!(
            dialog_marks("a", &aligned).unwrap().1,
            MarkSource::Alignment
        );
        let none = chunk(Vec::new(), MarksKind::Native, 0);
        assert_eq!(chunk_text("Ala ma. Kot śpi.", &none), "Kot śpi.");
        assert_eq!(chunk_text("Jedno zdanie", &none), "Jedno zdanie");
        assert!(dialog_marks("Kot śpi.", &none).is_none());
    }
}
