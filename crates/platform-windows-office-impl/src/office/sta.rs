//! Wątek COM STA dla Office z limitem czasu (Office potrafi wisieć na oknie dialogowym albo
//! uszkodzonym pliku). Zadanie = zamknięcie wysłane kanałem; wywołujący czeka `recv_timeout`.
//! Po przekroczeniu limitu wątek jest **porzucany** (kończy się, gdy wywołanie COM wróci), kolejne
//! zadanie dostaje świeży wątek; liczba wiszących wątków jest ograniczona (potem odmowa).
//! Obiekty COM żyją wyłącznie na wątku roboczym — przez kanał przechodzą tylko dane.

#![allow(unsafe_code)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use platform_apps_contract::{OfficeApp, OfficeError};
use windows::Win32::System::Com::{
    CLSIDFromProgID, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx,
    CoUninitialize,
};
use windows::core::PCWSTR;

/// Kontekst wątku STA (miejsce na stan per wątek; obiekty Office tworzone per operacja).
#[derive(Debug, Default)]
pub(crate) struct StaCtx {
    /// Liczba operacji na tym wątku (diagnostyka).
    pub(crate) operations: u64,
}

type Job = Box<dyn FnOnce(&mut StaCtx) + Send>;

struct Worker {
    tx: Sender<Job>,
    abandoned: Arc<AtomicBool>,
}

/// Właściciel wątku STA.
pub(crate) struct StaHost {
    worker: Mutex<Option<Worker>>,
    hung: Arc<AtomicUsize>,
    max_hung: usize,
    timeout_ms: u64,
}

impl std::fmt::Debug for StaHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StaHost")
            .field("hung", &self.hung.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

struct ExitGuard {
    abandoned: Arc<AtomicBool>,
    hung: Arc<AtomicUsize>,
}

impl Drop for ExitGuard {
    fn drop(&mut self) {
        if self.abandoned.load(Ordering::SeqCst) {
            self.hung.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

/// Czy ProgID aplikacji jest zarejestrowany (Office zainstalowany).
pub(crate) fn registered(app: OfficeApp) -> bool {
    let id: Vec<u16> = app.prog_id().encode_utf16().chain(Some(0)).collect();
    // SAFETY: napis zakończony zerem żyje do końca wywołania; funkcja tylko czyta rejestr klas.
    unsafe { CLSIDFromProgID(PCWSTR(id.as_ptr())) }.is_ok()
}

fn spawn(hung: Arc<AtomicUsize>) -> Result<Worker, OfficeError> {
    let (tx, rx) = mpsc::channel::<Job>();
    let abandoned = Arc::new(AtomicBool::new(false));
    let flag = abandoned.clone();
    std::thread::Builder::new()
        .name("alfa-office-sta".into())
        .spawn(move || {
            let _exit = ExitGuard {
                abandoned: flag,
                hung,
            };
            // SAFETY: nowy wątek; STA inicjalizowany raz, `CoUninitialize` tylko po sukcesie.
            let hr =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
            if hr.is_ok() {
                let mut ctx = StaCtx::default();
                while let Ok(job) = rx.recv() {
                    job(&mut ctx);
                    ctx.operations += 1;
                }
                // SAFETY: para do udanego `CoInitializeEx`; obiekty COM zwolnione w zadaniach.
                unsafe { CoUninitialize() };
            }
        })
        .map_err(|e| OfficeError::Document(format!("wątek Office: {e}")))?;
    Ok(Worker { tx, abandoned })
}

impl StaHost {
    /// Host z limitami.
    pub(crate) fn new(timeout_ms: u64, max_hung: usize) -> Self {
        Self {
            worker: Mutex::new(None),
            hung: Arc::new(AtomicUsize::new(0)),
            max_hung: max_hung.max(1),
            timeout_ms: timeout_ms.max(1_000),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Worker>> {
        self.worker.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn abandon(&self, still_running: bool) {
        if let Some(w) = self.lock().take()
            && still_running
        {
            self.hung.fetch_add(1, Ordering::SeqCst);
            w.abandoned.store(true, Ordering::SeqCst);
        }
    }

    /// Wykonuje zadanie na wątku STA z limitem czasu.
    pub(crate) fn call<R, F>(&self, op: &str, job: F) -> Result<R, OfficeError>
    where
        R: Send + 'static,
        F: FnOnce(&mut StaCtx) -> Result<R, OfficeError> + Send + 'static,
    {
        if self.hung.load(Ordering::SeqCst) >= self.max_hung {
            return Err(OfficeError::Timeout {
                op: format!("{op}: Office nie odpowiada — zamknij zawieszone okno Office"),
                ms: self.timeout_ms,
            });
        }
        let tx = {
            let mut guard = self.lock();
            if guard.is_none() {
                *guard = Some(spawn(self.hung.clone())?);
            }
            guard.as_ref().map(|w| w.tx.clone())
        };
        let closed = || OfficeError::Document("wątek Office zakończony".into());
        let tx = tx.ok_or_else(closed)?;
        let (rtx, rrx) = mpsc::sync_channel(1);
        tx.send(Box::new(move |ctx: &mut StaCtx| {
            let _ = rtx.send(job(ctx));
        }))
        .map_err(|_| closed())?;
        match rrx.recv_timeout(Duration::from_millis(self.timeout_ms)) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => {
                self.abandon(true);
                Err(OfficeError::Timeout {
                    op: format!("Office: {op}"),
                    ms: self.timeout_ms,
                })
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.abandon(false);
                Err(closed())
            }
        }
    }
}
