//! `DialogDriver` — łączy czysty automat z zasobem „głośnik” (`SpeakerLock`): wykonuje polecenia
//! `AcquireSpeaker`/`ReleaseSpeaker` i podaje automatowi odpowiedzi (`SpeakerGranted`/`Denied`).

use std::collections::VecDeque;

use voice_dialog_contract::{
    Command, DialogAutomaton, DialogEvent, DialogState, SpeakerLock, SpeakerOwner,
};

/// Maksymalna liczba zdarzeń wtórnych na jedno wejście (ochrona przed pętlą).
const MAX_FOLLOWUPS: usize = 8;

/// Sterownik automatu ze stanem i zasobem głośnika.
#[derive(Debug)]
pub struct DialogDriver<A, L> {
    automaton: A,
    lock: L,
    state: DialogState,
}

impl<A: DialogAutomaton, L: SpeakerLock> DialogDriver<A, L> {
    /// Nowy sterownik w stanie początkowym (`Idle`).
    pub fn new(automaton: A, lock: L) -> Self {
        Self {
            automaton,
            lock,
            state: DialogState::default(),
        }
    }

    /// Bieżący stan (dla UI i `agent-runtime`).
    pub fn state(&self) -> &DialogState {
        &self.state
    }

    /// Zasób głośnika.
    pub fn lock(&self) -> &L {
        &self.lock
    }

    /// Obsługuje zdarzenie; zwraca polecenia do wykonania (bez poleceń głośnika — te wykonuje sam).
    pub fn handle(&mut self, event: DialogEvent, now_ms: u64) -> Vec<Command> {
        let mut queue = VecDeque::from([event]);
        let mut out = Vec::new();
        let mut steps = 0;
        while let Some(ev) = queue.pop_front() {
            steps += 1;
            if steps > MAX_FOLLOWUPS + 1 {
                break;
            }
            let t = self.automaton.step(&self.state, &ev, now_ms);
            self.state = t.state;
            for c in t.commands {
                match c {
                    Command::AcquireSpeaker { persona, utterance } => {
                        let owner = SpeakerOwner {
                            persona: persona.clone(),
                            utterance,
                        };
                        queue.push_back(match self.lock.try_acquire(&owner) {
                            Ok(()) => DialogEvent::SpeakerGranted { persona, utterance },
                            Err(_) => DialogEvent::SpeakerDenied { persona, utterance },
                        });
                    }
                    Command::ReleaseSpeaker { persona, utterance } => {
                        self.lock.release(&SpeakerOwner { persona, utterance });
                    }
                    other => out.push(other),
                }
            }
        }
        out
    }
}
