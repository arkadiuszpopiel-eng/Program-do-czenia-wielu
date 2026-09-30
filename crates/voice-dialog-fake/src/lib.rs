//! Atrapy modułu `voice-dialog` (SPEC „Fake”): uproszczony automat `FakeDialog` (natychmiastowy
//! twardy stop przy mowie użytkownika, bez backchannelu i fillerów, intencja zawsze „korekta”),
//! `FakeSpeakerLock` (zasób głośnika w pamięci z dziennikiem), `ScriptedClassifier` (intencje
//! z adnotacji) i `UniformAligner` (słowa rozłożone równo w czasie audio). Deterministyczne.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod automaton;
mod ctx;

pub use automaton::FakeDialog;

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use voice_dialog_contract::{
    AlignError, IntentResult, InterruptClassifier, InterruptContext, InterruptIntent, SpeakerBusy,
    SpeakerLock, SpeakerOwner, WordAligner, WordMark,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Wpis dziennika zasobu głośnika.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockCall {
    /// Udane przejęcie.
    Acquired(SpeakerOwner),
    /// Odmowa (zajęty).
    Denied(SpeakerOwner),
    /// Zwolnienie (czy faktycznie zwolniono).
    Released(SpeakerOwner, bool),
}

/// Zasób „głośnik” w pamięci (do czasu `scheduler-lite`).
#[derive(Debug, Default)]
pub struct FakeSpeakerLock {
    holder: Mutex<Option<SpeakerOwner>>,
    log: Mutex<Vec<LockCall>>,
}

impl FakeSpeakerLock {
    /// Wolny głośnik.
    pub fn new() -> Self {
        Self::default()
    }

    /// Zajmuje głośnik z zewnątrz (np. inna sesja/agentka).
    pub fn occupy(&self, owner: SpeakerOwner) {
        *lock(&self.holder) = Some(owner);
    }

    /// Dziennik wywołań.
    pub fn log(&self) -> Vec<LockCall> {
        lock(&self.log).clone()
    }
}

impl SpeakerLock for FakeSpeakerLock {
    fn try_acquire(&self, owner: &SpeakerOwner) -> Result<(), SpeakerBusy> {
        let mut holder = lock(&self.holder);
        match holder.as_ref() {
            Some(h) if h != owner => {
                lock(&self.log).push(LockCall::Denied(owner.clone()));
                Err(SpeakerBusy { holder: h.clone() })
            }
            _ => {
                *holder = Some(owner.clone());
                lock(&self.log).push(LockCall::Acquired(owner.clone()));
                Ok(())
            }
        }
    }

    fn release(&self, owner: &SpeakerOwner) -> bool {
        let mut holder = lock(&self.holder);
        let released = holder.as_ref() == Some(owner);
        if released {
            *holder = None;
        }
        lock(&self.log).push(LockCall::Released(owner.clone(), released));
        released
    }

    fn holder(&self) -> Option<SpeakerOwner> {
        lock(&self.holder).clone()
    }
}

/// Klasyfikator z adnotacji: wypowiedź (małe litery, przycięta) → intencja; domyślnie korekta.
#[derive(Debug, Default)]
pub struct ScriptedClassifier {
    script: Mutex<BTreeMap<String, InterruptIntent>>,
    calls: Mutex<Vec<String>>,
}

impl ScriptedClassifier {
    /// Pusty skrypt.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adnotacja.
    pub fn script(&self, utterance: &str, intent: InterruptIntent) {
        lock(&self.script).insert(utterance.trim().to_lowercase(), intent);
    }

    /// Wypowiedzi przekazane do klasyfikacji.
    pub fn calls(&self) -> Vec<String> {
        lock(&self.calls).clone()
    }
}

impl InterruptClassifier for ScriptedClassifier {
    fn classify(&self, ctx: &InterruptContext<'_>) -> IntentResult {
        let key = ctx.utterance.trim().to_lowercase();
        lock(&self.calls).push(key.clone());
        match lock(&self.script).get(&key) {
            Some(intent) => IntentResult {
                intent: *intent,
                confidence: 1.0,
            },
            None => IntentResult {
                intent: InterruptIntent::Correction,
                confidence: 0.5,
            },
        }
    }
}

/// Alignment atrapy: słowa rozłożone proporcjonalnie do długości w czasie audio.
#[derive(Debug, Clone, Copy, Default)]
pub struct UniformAligner;

impl WordAligner for UniformAligner {
    fn align(
        &self,
        text: &str,
        audio: &[f32],
        sample_rate: u32,
    ) -> Result<Vec<WordMark>, AlignError> {
        if sample_rate == 0 {
            return Err(AlignError {
                reason: "częstotliwość 0".into(),
            });
        }
        let total_ms = u64::try_from(audio.len()).unwrap_or(0) * 1000 / u64::from(sample_rate);
        let chars: Vec<char> = text.chars().collect();
        let n = u64::try_from(chars.len().max(1)).unwrap_or(1);
        let mut marks = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            let at = |c: usize| u64::try_from(c).unwrap_or(0) * total_ms / n;
            marks.push(WordMark {
                char_start: start,
                char_end: i,
                start_ms: at(start),
                end_ms: at(i),
            });
        }
        Ok(marks)
    }
}
