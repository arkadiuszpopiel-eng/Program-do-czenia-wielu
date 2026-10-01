//! UI Automation na dedykowanym wątku COM MTA z limitem czasu (PLAN §7.3: UIA potrafi wisieć na
//! zawieszonej aplikacji). Żądanie = zamknięcie wysłane kanałem; wywołujący czeka
//! `recv_timeout`. Po przekroczeniu limitu wątek jest **porzucany** (kanał zamknięty — zakończy
//! się sam, gdy wywołanie COM wróci), a kolejne żądanie dostaje świeży wątek. Liczba porzuconych,
//! jeszcze wiszących wątków jest ograniczona — powyżej limitu UIA odmawia od razu (`Timeout`).
//! Dodatkowo `IUIAutomation2` dostaje własne limity połączenia i transakcji.
//!
//! Obiekty COM żyją tylko na wątku roboczym (`UiaCtx`); przez kanał przechodzą wyłącznie dane.

#![allow(unsafe_code)]

mod act;
mod fields;
mod read;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use platform_contract::{GuiError, PlatformError};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationCacheRequest,
    IUIAutomationElement, IUIAutomationTreeWalker,
};
use windows::core::Interface;

pub(crate) use act::act;
pub(crate) use fields::{password_rects, read_text};
pub(crate) use read::{find, node_of_element, tree};

use crate::win::win_error;

/// Limit wpisów pamięci podręcznej elementów (potem czyszczona).
const MAX_CACHED_ELEMENTS: usize = 10_000;

/// Kontekst wątku UIA (obiekty COM — tylko na tym wątku).
pub(crate) struct UiaCtx {
    pub(crate) automation: IUIAutomation,
    pub(crate) request: IUIAutomationCacheRequest,
    pub(crate) walker: IUIAutomationTreeWalker,
    cache: HashMap<(u64, Vec<i32>), IUIAutomationElement>,
}

impl UiaCtx {
    fn new(connection_ms: u32, transaction_ms: u32) -> Result<Self, GuiError> {
        // SAFETY: tworzenie obiektu UIA w zainicjalizowanym apartamencie MTA tego wątku.
        let automation: IUIAutomation = unsafe {
            CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)
                .or_else(|_| CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER))
        }
        .map_err(|e| win_error("CoCreateInstance(CUIAutomation)", &e))?;
        if let Ok(a2) = automation.cast::<IUIAutomation2>() {
            // SAFETY: ustawienie limitów czasu klienta UIA (Windows 8+).
            unsafe {
                let _ = a2.SetConnectionTimeout(connection_ms);
                let _ = a2.SetTransactionTimeout(transaction_ms);
            }
        }
        let request = read::cache_request(&automation)?;
        // SAFETY: widok kontrolek (bez węzłów czysto prezentacyjnych).
        let walker = unsafe { automation.ControlViewWalker() }
            .map_err(|e| win_error("ControlViewWalker", &e))?;
        Ok(Self {
            automation,
            request,
            walker,
            cache: HashMap::new(),
        })
    }

    pub(crate) fn remember(&mut self, window: u64, runtime_id: Vec<i32>, el: IUIAutomationElement) {
        if self.cache.len() >= MAX_CACHED_ELEMENTS {
            self.cache.clear();
        }
        self.cache.insert((window, runtime_id), el);
    }

    pub(crate) fn cached(&self, window: u64, runtime_id: &[i32]) -> Option<IUIAutomationElement> {
        self.cache.get(&(window, runtime_id.to_vec())).cloned()
    }

    pub(crate) fn forget(&mut self, window: u64, runtime_id: &[i32]) {
        self.cache.remove(&(window, runtime_id.to_vec()));
    }
}

type Job = Box<dyn FnOnce(&mut UiaCtx) + Send>;

struct Worker {
    tx: Sender<Job>,
    abandoned: Arc<AtomicBool>,
}

/// Właściciel wątku UIA.
pub(crate) struct UiaHost {
    worker: Mutex<Option<Worker>>,
    hung: Arc<AtomicUsize>,
    max_hung: usize,
    call_timeout_ms: u64,
}

impl std::fmt::Debug for UiaHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiaHost")
            .field("hung", &self.hung.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

/// Zmniejsza licznik wiszących wątków przy wyjściu porzuconego wątku (także po panice).
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

fn spawn(hung: Arc<AtomicUsize>, call_timeout_ms: u64) -> Result<Worker, GuiError> {
    let (tx, rx) = mpsc::channel::<Job>();
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), GuiError>>(1);
    let abandoned = Arc::new(AtomicBool::new(false));
    let flag = abandoned.clone();
    let ms = u32::try_from(call_timeout_ms).unwrap_or(u32::MAX);
    std::thread::Builder::new()
        .name("alfa-uia-mta".into())
        .spawn(move || {
            let _exit = ExitGuard {
                abandoned: flag,
                hung,
            };
            // SAFETY: nowy wątek; COM inicjalizowany raz, `CoUninitialize` tylko po sukcesie.
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if hr.is_err() {
                let err = windows::core::Error::from_hresult(hr);
                let _ = ready_tx.send(Err(win_error("CoInitializeEx", &err)));
            } else {
                match UiaCtx::new(ms, ms) {
                    Ok(mut ctx) => {
                        let _ = ready_tx.send(Ok(()));
                        while let Ok(job) = rx.recv() {
                            job(&mut ctx);
                        }
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
                // SAFETY: para do udanego `CoInitializeEx`; obiekty COM (`ctx`) już zwolnione.
                unsafe { CoUninitialize() };
            }
        })
        .map_err(|e| GuiError::Platform(PlatformError::Io(format!("wątek UIA: {e}"))))?;
    match ready_rx.recv_timeout(Duration::from_millis(call_timeout_ms.max(1_000))) {
        Ok(Ok(())) => Ok(Worker { tx, abandoned }),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(GuiError::Timeout {
            op: "start UIA".into(),
            ms: call_timeout_ms,
        }),
    }
}

impl UiaHost {
    /// Host z limitami.
    pub(crate) fn new(call_timeout_ms: u64, max_hung: usize) -> Self {
        Self {
            worker: Mutex::new(None),
            hung: Arc::new(AtomicUsize::new(0)),
            max_hung: max_hung.max(1),
            call_timeout_ms,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<Worker>> {
        self.worker.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Porzuca bieżący wątek; `still_running` = wątek wisi w wywołaniu (liczony do limitu).
    fn abandon(&self, still_running: bool) {
        if let Some(w) = self.lock().take()
            && still_running
        {
            self.hung.fetch_add(1, Ordering::SeqCst);
            w.abandoned.store(true, Ordering::SeqCst);
        }
    }

    /// Wykonuje zadanie na wątku UIA z limitem czasu.
    pub(crate) fn call<R, F>(&self, op: &str, timeout_ms: u64, job: F) -> Result<R, GuiError>
    where
        R: Send + 'static,
        F: FnOnce(&mut UiaCtx) -> Result<R, GuiError> + Send + 'static,
    {
        if self.hung.load(Ordering::SeqCst) >= self.max_hung {
            return Err(GuiError::Timeout {
                op: format!("{op}: UIA zawieszone w innych aplikacjach — spróbuj za chwilę"),
                ms: timeout_ms,
            });
        }
        let tx = {
            let mut guard = self.lock();
            if guard.is_none() {
                *guard = Some(spawn(self.hung.clone(), self.call_timeout_ms)?);
            }
            guard.as_ref().map(|w| w.tx.clone())
        };
        let closed = || GuiError::Platform(PlatformError::Io("wątek UIA zakończony".into()));
        let tx = tx.ok_or_else(closed)?;
        let (rtx, rrx) = mpsc::sync_channel(1);
        tx.send(Box::new(move |ctx: &mut UiaCtx| {
            let _ = rtx.send(job(ctx));
        }))
        .map_err(|_| closed())?;
        match rrx.recv_timeout(Duration::from_millis(timeout_ms)) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => {
                self.abandon(true);
                Err(GuiError::Timeout {
                    op: format!("UIA: {op}"),
                    ms: timeout_ms,
                })
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.abandon(false);
                Err(closed())
            }
        }
    }
}
