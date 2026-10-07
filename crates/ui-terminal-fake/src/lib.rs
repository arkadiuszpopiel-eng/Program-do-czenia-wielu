//! Atrapa `ui-terminal` (docs/modules/ui-terminal/SPEC.md, sekcja „Fake”): sesje w pamięci,
//! wejście odbijane jako wyjście (echo), skryptowane wyjście (`emit`), zamknięcie zgłasza koniec
//! do odbiorcy. Bez procesów i bez zapisu treści poza odbiorcą (jak implementacja).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use platform_contract::PtySize;
use ui_terminal_contract::{
    MAX_INPUT_BYTES, OpenRequest, TerminalError, TerminalId, TerminalInfo, TerminalProfile,
    TerminalService, TerminalSink, UserGesture,
};

struct Fake {
    profile: TerminalProfile,
    sink: Arc<dyn TerminalSink>,
}

#[derive(Default)]
struct State {
    sessions: BTreeMap<TerminalId, Fake>,
    opened: Vec<TerminalProfile>,
    next: u64,
}

/// Atrapa usługi terminali.
#[derive(Default)]
pub struct FakeTerminals {
    state: Mutex<State>,
    max_sessions: usize,
}

impl std::fmt::Debug for FakeTerminals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeTerminals")
            .field("sessions", &self.lock().sessions.len())
            .finish_non_exhaustive()
    }
}

impl FakeTerminals {
    /// Atrapa z limitem sesji.
    pub fn new(max_sessions: usize) -> Self {
        Self {
            state: Mutex::new(State::default()),
            max_sessions: max_sessions.max(1),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Wyjście „procesu” sesji (np. ekran logowania CLI).
    pub fn emit(&self, id: TerminalId, bytes: &[u8]) {
        let sink = self.lock().sessions.get(&id).map(|s| s.sink.clone());
        if let Some(sink) = sink {
            sink.output(id, bytes);
        }
    }

    /// Otwarte profile (historia).
    pub fn opened(&self) -> Vec<TerminalProfile> {
        self.lock().opened.clone()
    }
}

impl TerminalService for FakeTerminals {
    fn open(
        &self,
        request: OpenRequest,
        _gesture: &UserGesture,
        sink: Arc<dyn TerminalSink>,
    ) -> Result<TerminalId, TerminalError> {
        request
            .size
            .validate()
            .map_err(|e| TerminalError::Platform(e.to_string()))?;
        let mut s = self.lock();
        if s.sessions.len() >= self.max_sessions {
            return Err(TerminalError::Limit(self.max_sessions));
        }
        s.next += 1;
        let id = TerminalId(s.next);
        s.sessions.insert(
            id,
            Fake {
                profile: request.profile,
                sink,
            },
        );
        s.opened.push(request.profile);
        Ok(id)
    }

    fn input(
        &self,
        id: TerminalId,
        data: &[u8],
        _gesture: &UserGesture,
    ) -> Result<(), TerminalError> {
        if data.len() > MAX_INPUT_BYTES {
            return Err(TerminalError::InputTooLarge);
        }
        let sink = self
            .lock()
            .sessions
            .get(&id)
            .map(|s| s.sink.clone())
            .ok_or(TerminalError::NotFound(id.0))?;
        sink.output(id, data);
        Ok(())
    }

    fn resize(&self, id: TerminalId, size: PtySize) -> Result<(), TerminalError> {
        size.validate()
            .map_err(|e| TerminalError::Platform(e.to_string()))?;
        self.lock()
            .sessions
            .get(&id)
            .map(|_| ())
            .ok_or(TerminalError::NotFound(id.0))
    }

    fn close(&self, id: TerminalId) -> Result<(), TerminalError> {
        let fake = self
            .lock()
            .sessions
            .remove(&id)
            .ok_or(TerminalError::NotFound(id.0))?;
        fake.sink.exited(id, Some(-1));
        Ok(())
    }

    fn list(&self) -> Vec<TerminalInfo> {
        self.lock()
            .sessions
            .iter()
            .map(|(id, f)| TerminalInfo {
                id: *id,
                profile: f.profile,
                pid: 0,
                alive: true,
            })
            .collect()
    }
}
