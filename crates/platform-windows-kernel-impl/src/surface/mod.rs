//! `ApprovalSurfacePort`: natywne okno Broker-UI na dedykowanym wątku z pętlą komunikatów
//! (Win32 bez WebView i bez renderowania HTML), zdarzenia w kolejce czytanej przez logikę
//! `broker-ui`. Poza Windows: `Unsupported`.

#[cfg(windows)]
mod input;
#[cfg(windows)]
mod thread;
#[cfg(windows)]
mod win;

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use platform_contract::{ApprovalSurfacePort, PlatformError, SurfaceEvent, SurfaceView};

/// Maksymalna liczba nieodebranych zdarzeń (najstarsze odrzucane).
const CAPACITY: usize = 256;

/// Chwila w ms od epoki UNIX (ta sama domena czasu co zegar Brokera).
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Kolejka zdarzeń okna (wątek okna dopisuje, logika odbiera).
#[derive(Debug, Default)]
pub(crate) struct EventQueue {
    queue: Mutex<VecDeque<SurfaceEvent>>,
    ready: Condvar,
}

impl EventQueue {
    fn lock(&self) -> MutexGuard<'_, VecDeque<SurfaceEvent>> {
        self.queue.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Dopisuje zdarzenie.
    pub(crate) fn push(&self, event: SurfaceEvent) {
        let mut q = self.lock();
        if q.len() >= CAPACITY {
            q.pop_front();
        }
        q.push_back(event);
        self.ready.notify_all();
    }

    fn wait(&self, timeout: Duration) -> Option<SurfaceEvent> {
        let guard = self.lock();
        let (mut q, _) = self
            .ready
            .wait_timeout_while(guard, timeout, |q| q.is_empty())
            .unwrap_or_else(|p| p.into_inner());
        q.pop_front()
    }
}

/// Natywne okno zatwierdzeń (topmost, duże przyciski, `Enter` niczego nie zatwierdza, `Esc` =
/// odmowa, okno nie kradnie fokusu poza `take_focus`). Wejście oznaczane jako wstrzyknięte na
/// podstawie `GetCurrentInputMessageSource` i flag hooków `WH_KEYBOARD_LL`/`WH_MOUSE_LL`.
#[derive(Debug, Default)]
pub struct WinApprovalSurface {
    events: Arc<EventQueue>,
    #[cfg(windows)]
    thread: Mutex<Option<thread::SurfaceThread>>,
}

impl WinApprovalSurface {
    /// Nowe okno (wątek startuje przy pierwszym `present`).
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(windows)]
    fn send(&self, request: thread::Request) -> Result<(), PlatformError> {
        let mut guard = self.thread.lock().unwrap_or_else(|p| p.into_inner());
        if guard.is_none() {
            *guard = Some(thread::SurfaceThread::start(self.events.clone())?);
        }
        match guard.as_ref() {
            Some(t) => t.send(request),
            None => Err(PlatformError::Io("wątek okna niedostępny".into())),
        }
    }
}

impl ApprovalSurfacePort for WinApprovalSurface {
    fn present(&self, view: &SurfaceView) -> Result<(), PlatformError> {
        view.validate()?;
        #[cfg(windows)]
        let shown = self.send(thread::Request::Present(Box::new(view.clone())));
        #[cfg(not(windows))]
        let shown = Err(PlatformError::Unsupported(
            "okno Broker-UI: tylko Windows".into(),
        ));
        shown
    }

    fn dismiss(&self) -> Result<(), PlatformError> {
        #[cfg(windows)]
        let done = self.send(thread::Request::Dismiss);
        #[cfg(not(windows))]
        let done = Ok(());
        done
    }

    fn next_event(&self, timeout_ms: u32) -> Option<SurfaceEvent> {
        self.events.wait(Duration::from_millis(timeout_ms.into()))
    }
}
