//! Wątek okna Broker-UI: start z czekaniem na gotowość kolejki komunikatów, żądania przez kanał
//! + `PostThreadMessageW`, zakończenie `WM_QUIT` w `Drop`.

#![allow(unsafe_code)]

use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

use platform_contract::{PlatformError, SurfaceView};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP, WM_QUIT};

use super::EventQueue;
use super::win::run;
use crate::win::win_error;

/// Komunikat „są żądania w kanale”.
pub(super) const WM_REQUEST: u32 = WM_APP + 2;

/// Żądanie do wątku okna.
pub(crate) enum Request {
    Present(Box<SurfaceView>),
    Dismiss,
}

/// Uchwyt wątku okna; `Drop` kończy pętlę (`WM_QUIT`).
#[derive(Debug)]
pub(crate) struct SurfaceThread {
    thread_id: u32,
    requests: Sender<Request>,
    join: Option<JoinHandle<()>>,
}

fn post(thread_id: u32, message: u32) -> Result<(), PlatformError> {
    // SAFETY: komunikat do kolejki wątku, bez wskaźników.
    unsafe { PostThreadMessageW(thread_id, message, WPARAM(0), LPARAM(0)) }
        .map_err(|e| win_error("PostThreadMessageW", &e))
}

impl SurfaceThread {
    pub(crate) fn start(events: Arc<EventQueue>) -> Result<Self, PlatformError> {
        let (requests, inbox) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let join = std::thread::Builder::new()
            .name("alfa-broker-ui-okno".into())
            .spawn(move || run(&inbox, events, &ready_tx))
            .map_err(|e| PlatformError::Io(format!("wątek okna: {e}")))?;
        let thread_id = ready_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| PlatformError::Io("wątek okna nie odpowiedział".into()))?;
        let join = Some(join);
        Ok(Self {
            thread_id,
            requests,
            join,
        })
    }

    pub(crate) fn send(&self, request: Request) -> Result<(), PlatformError> {
        let closed = |_| PlatformError::Io("wątek okna zakończony".into());
        self.requests.send(request).map_err(closed)?;
        post(self.thread_id, WM_REQUEST)
    }
}

impl Drop for SurfaceThread {
    fn drop(&mut self) {
        if post(self.thread_id, WM_QUIT).is_ok()
            && let Some(join) = self.join.take()
        {
            let _ = join.join();
        }
    }
}
