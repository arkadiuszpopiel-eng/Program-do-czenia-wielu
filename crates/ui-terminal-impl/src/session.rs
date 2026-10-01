//! Sesja terminala: wątek wyjścia (VT → `TerminalSink`, nic więcej) i wątek nadzoru (koniec
//! procesu → zamknięcie pseudokonsoli, żeby wyjście dostało EOF i UI zobaczyło „zakończono”).

use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use platform_contract::{PlatformError, PtySession};
use ui_terminal_contract::{EVENT_EXITED, TerminalId, TerminalInfo, TerminalProfile, TerminalSink};

use crate::Publisher;

/// Odstęp sprawdzania, czy proces się zakończył.
const WATCH_EVERY: Duration = Duration::from_millis(200);

/// Otwarta sesja.
pub(crate) struct Session {
    id: TerminalId,
    profile: TerminalProfile,
    pid: u32,
    pty: Arc<dyn PtySession>,
    closed: Arc<AtomicBool>,
}

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) -> Result<(), PlatformError> {
    std::thread::Builder::new()
        .name(name.into())
        .spawn(work)
        .map(|_| ())
        .map_err(|e| PlatformError::Io(format!("wątek terminala: {e}")))
}

impl Session {
    /// Startuje wątki wyjścia i nadzoru.
    pub(crate) fn start(
        id: TerminalId,
        profile: TerminalProfile,
        pty: Arc<dyn PtySession>,
        sink: Arc<dyn TerminalSink>,
        publisher: Publisher,
    ) -> Result<Self, PlatformError> {
        let mut out = pty.take_output()?;
        let closed = Arc::new(AtomicBool::new(false));
        let reader_pty = pty.clone();
        spawn("alfa-terminal-wyjscie", move || {
            let mut buf = [0u8; 8192];
            loop {
                match out.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => sink.output(id, &buf[..n]),
                }
            }
            buf.fill(0);
            let code = reader_pty.exit_code().ok().flatten();
            sink.exited(id, code);
            publisher.publish(
                EVENT_EXITED,
                serde_json::json!({ "terminal": id.0, "code": code }),
            );
        })
        .inspect_err(|_| {
            let _ = pty.close();
        })?;
        let (watch_pty, watch_closed) = (pty.clone(), closed.clone());
        spawn("alfa-terminal-nadzor", move || {
            while !watch_closed.load(Ordering::SeqCst) {
                if matches!(watch_pty.exit_code(), Ok(Some(_)) | Err(_)) {
                    let _ = watch_pty.close();
                    break;
                }
                std::thread::sleep(WATCH_EVERY);
            }
        })
        .inspect_err(|_| {
            let _ = pty.close();
        })?;
        Ok(Self {
            id,
            profile,
            pid: pty.pid(),
            pty,
            closed,
        })
    }

    pub(crate) fn pty(&self) -> Arc<dyn PtySession> {
        self.pty.clone()
    }

    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn info(&self) -> TerminalInfo {
        TerminalInfo {
            id: self.id,
            profile: self.profile,
            pid: self.pid,
            alive: matches!(self.pty.exit_code(), Ok(None)),
        }
    }

    /// Zamyka pseudokonsolę i zabija drzewo procesów.
    pub(crate) fn close(self) -> Result<(), PlatformError> {
        self.closed.store(true, Ordering::SeqCst);
        self.pty.close()
    }
}
