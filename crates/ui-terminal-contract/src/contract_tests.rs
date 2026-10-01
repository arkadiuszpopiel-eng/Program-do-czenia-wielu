//! Współdzielone testy kontraktowe `TerminalService` (feature `contract-tests`), uruchamiane na
//! `-impl` (z atrapą pseudokonsoli) i `-fake`: cykl życia, limity, zamknięcie z powiadomieniem.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::{
    MAX_INPUT_BYTES, OpenRequest, PtySize, TerminalError, TerminalId, TerminalProfile,
    TerminalService, TerminalSink, ui_only,
};

/// Odbiorca zapisujący strumień (do asercji).
#[derive(Debug, Default)]
pub struct RecordingSink {
    output: Mutex<Vec<(TerminalId, Vec<u8>)>>,
    exits: Mutex<Vec<(TerminalId, Option<i32>)>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl RecordingSink {
    /// Całe wyjście sesji.
    pub fn output_of(&self, id: TerminalId) -> Vec<u8> {
        lock(&self.output)
            .iter()
            .filter(|(i, _)| *i == id)
            .flat_map(|(_, c)| c.clone())
            .collect()
    }

    /// Czy zgłoszono koniec sesji.
    pub fn exited(&self, id: TerminalId) -> bool {
        lock(&self.exits).iter().any(|(i, _)| *i == id)
    }

    /// Czeka (do 5 s) na warunek.
    pub fn wait(&self, mut cond: impl FnMut(&Self) -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if cond(self) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        cond(self)
    }
}

impl TerminalSink for RecordingSink {
    fn output(&self, id: TerminalId, chunk: &[u8]) {
        lock(&self.output).push((id, chunk.to_vec()));
    }

    fn exited(&self, id: TerminalId, code: Option<i32>) {
        lock(&self.exits).push((id, code));
    }
}

fn request() -> OpenRequest {
    OpenRequest {
        profile: TerminalProfile::Shell,
        size: PtySize::default(),
        cwd: None,
    }
}

/// Cały zestaw (usługa z limitem `max_sessions` ≥ 1 i programem powłoki).
pub fn run_all<S: TerminalService + ?Sized>(service: &S, max_sessions: usize) {
    let sink = Arc::new(RecordingSink::default());
    let g = ui_only::user_gesture();
    let id = service
        .open(request(), &g, sink.clone())
        .unwrap_or_else(|e| panic!("open: {e}"));
    assert!(service.list().iter().any(|t| t.id == id && t.alive));
    service
        .input(id, b"echo kontrakt\r\n", &g)
        .unwrap_or_else(|e| panic!("input: {e}"));
    assert_eq!(
        service.input(id, &vec![b'x'; MAX_INPUT_BYTES + 1], &g),
        Err(TerminalError::InputTooLarge)
    );
    service
        .resize(id, PtySize { cols: 80, rows: 24 })
        .unwrap_or_else(|e| panic!("resize: {e}"));
    assert!(service.resize(id, PtySize { cols: 0, rows: 0 }).is_err());
    let missing = TerminalId(u64::MAX);
    assert_eq!(
        service.input(missing, b"x", &g),
        Err(TerminalError::NotFound(u64::MAX))
    );
    assert_eq!(
        service.close(missing),
        Err(TerminalError::NotFound(u64::MAX))
    );
    service.close(id).unwrap_or_else(|e| panic!("close: {e}"));
    assert!(
        sink.wait(|s| s.exited(id)),
        "zamknięcie zgłasza koniec sesji do UI"
    );
    assert!(!service.list().iter().any(|t| t.id == id));
    assert_eq!(service.close(id), Err(TerminalError::NotFound(id.0)));
    let mut open = Vec::new();
    for _ in 0..max_sessions {
        open.push(
            service
                .open(request(), &g, sink.clone())
                .unwrap_or_else(|e| panic!("open: {e}")),
        );
    }
    assert_eq!(
        service.open(request(), &g, sink.clone()),
        Err(TerminalError::Limit(max_sessions))
    );
    for id in open {
        service.close(id).unwrap_or_else(|e| panic!("close: {e}"));
    }
}
