//! Symulator na wirtualnym zegarze dla testów automatu.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use voice_dialog_contract::{
    ActivationSource, Command, DialogAutomaton, DialogEvent, DialogPhase, DialogState, UtteranceId,
    WordMark,
};
use voice_persona_contract::PersonaId;

/// Automat + stan + dziennik poleceń z czasem.
pub struct Sim<A> {
    pub a: A,
    pub s: DialogState,
    pub now: u64,
    pub log: Vec<(u64, Command)>,
}

impl<A: DialogAutomaton> Sim<A> {
    pub fn new(a: A) -> Self {
        Self {
            a,
            s: DialogState::default(),
            now: 0,
            log: Vec::new(),
        }
    }

    /// Zdarzenie w chwili `t` (czas nie cofa się).
    pub fn at(&mut self, t: u64, e: DialogEvent) -> Vec<Command> {
        self.now = self.now.max(t);
        let tr = self.a.step(&self.s, &e, self.now);
        self.s = tr.state;
        self.log
            .extend(tr.commands.iter().map(|c| (self.now, c.clone())));
        tr.commands
    }

    /// Tyka co `step` ms do `until` (włącznie).
    pub fn tick_until(&mut self, until: u64, step: u64) {
        while self.now + step <= until {
            let t = self.now + step;
            self.at(t, DialogEvent::Tick);
        }
    }

    /// Polecenia od chwili `from`.
    pub fn since(&self, from: u64) -> Vec<&Command> {
        self.log
            .iter()
            .filter(|(t, _)| *t >= from)
            .map(|(_, c)| c)
            .collect()
    }

    /// Pierwsza chwila polecenia spełniającego predykat.
    pub fn first(&self, pred: impl Fn(&Command) -> bool) -> Option<u64> {
        self.log.iter().find(|(_, c)| pred(c)).map(|(t, _)| *t)
    }

    pub fn count(&self, pred: impl Fn(&Command) -> bool) -> usize {
        self.log.iter().filter(|(_, c)| pred(c)).count()
    }

    /// Doprowadza do `Speaking` Alfy; zwraca id wypowiedzi. Tura użytkownika kończy się o `t0`.
    pub fn speak(&mut self, t0: u64, user: &str) -> UtteranceId {
        if self.s.phase == DialogPhase::Idle {
            self.at(
                t0,
                DialogEvent::Activate {
                    source: ActivationSource::PushToTalk,
                },
            );
        }
        self.at(t0, DialogEvent::VadSpeechStart);
        self.at(t0 + 300, DialogEvent::UserPartial { text: user.into() });
        self.at(t0 + 500, DialogEvent::VadSpeechEnd);
        self.at(t0 + 700, DialogEvent::TurnEnded);
        let cmds = self.at(
            t0 + 900,
            DialogEvent::ResponseReady {
                persona: PersonaId::alfa(),
            },
        );
        let id = cmds
            .iter()
            .find_map(|c| match c {
                Command::AcquireSpeaker { utterance, .. } => Some(*utterance),
                _ => None,
            })
            .expect("AcquireSpeaker");
        self.at(
            t0 + 900,
            DialogEvent::SpeakerGranted {
                persona: PersonaId::alfa(),
                utterance: id,
            },
        );
        id
    }

    /// Kolejkuje wypowiedź ze słów (każde słowo `word_ms` + przerwa `gap_ms`) jako jeden fragment,
    /// opcjonalnie ze znacznikami słów. Zwraca (tekst, znaczniki) do wyznaczania prawdy.
    pub fn queue_words(
        &mut self,
        id: UtteranceId,
        words: &[&str],
        word_ms: u64,
        gap_ms: u64,
        with_marks: bool,
    ) -> (String, Vec<WordMark>) {
        let text = words.join(" ");
        let mut marks = Vec::new();
        let (mut ch, mut t) = (0usize, 0u64);
        for w in words {
            let len = w.chars().count();
            marks.push(WordMark {
                char_start: ch,
                char_end: ch + len,
                start_ms: t,
                end_ms: t + word_ms,
            });
            ch += len + 1;
            t += word_ms + gap_ms;
        }
        let now = self.now;
        self.at(
            now,
            DialogEvent::TtsChunkQueued {
                utterance: id,
                text: text.clone(),
                audio_ms: t,
            },
        );
        if with_marks {
            self.at(
                now,
                DialogEvent::TtsWordMarks {
                    utterance: id,
                    chunk: 0,
                    marks: marks.clone(),
                    source: voice_dialog_contract::MarkSource::Tts,
                },
            );
        }
        (text, marks)
    }

    /// Postęp odtwarzania: audio wypowiedzi od `start` z opóźnieniem urządzenia `latency`, co 20 ms do `until`.
    pub fn play(&mut self, id: UtteranceId, start: u64, until: u64, latency: u64) {
        let mut t = self.now.max(start);
        while t <= until {
            let played_ms = t - start + latency;
            self.at(
                t,
                DialogEvent::PlaybackProgress {
                    utterance: id,
                    played_samples: played_ms * 48,
                    sample_rate: 48_000,
                    device_latency_ms: latency,
                },
            );
            t += 20;
        }
    }
}

/// Prosty deterministyczny generator (LCG) do scenariuszy syntetycznych.
pub struct Lcg(pub u64);

impl Lcg {
    pub fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next() % (hi - lo + 1)
    }
}
