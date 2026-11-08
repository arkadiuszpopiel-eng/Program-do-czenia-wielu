//! Hook aktywności użytkownika (współdzielenie wejścia, PLAN §7.4): `WH_KEYBOARD_LL` +
//! `WH_MOUSE_LL` na dedykowanym wątku z pętlą komunikatów; każde zdarzenie **niewstrzyknięte**
//! (bez `LLKHF_INJECTED`/`LLMHF_INJECTED`, także ruch myszy) zapisuje chwilę ostatniego
//! fizycznego wejścia. Wejście syntetyczne Alfy ma flagę wstrzyknięcia, więc nie przerywa samo
//! siebie. Wywołania zwrotne tylko zapisują liczbę (limit `LowLevelHooksTimeout`).

#![allow(unsafe_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use platform_contract::{GuiError, HookOrigin, PlatformError};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
    PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_QUIT, WM_USER,
};

use crate::win::now_ms;

/// Ostatnie fizyczne wejście (ms od epoki; 0 = brak). Wspólne dla procesu (jeden hook).
static LAST_PHYSICAL: AtomicU64 = AtomicU64::new(0);

unsafe extern "system" fn key_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && lparam.0 != 0 {
        // SAFETY: dla `HC_ACTION` `lparam` wskazuje na `KBDLLHOOKSTRUCT`.
        let flags = unsafe { (*(lparam.0 as *const KBDLLHOOKSTRUCT)).flags.0 };
        if !HookOrigin::from_keyboard_flags(flags).is_injected() {
            LAST_PHYSICAL.store(now_ms(), Ordering::SeqCst);
        }
    }
    // SAFETY: przekazanie dalej w łańcuchu hooków (hook nie blokuje wejścia).
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && lparam.0 != 0 {
        // SAFETY: dla `HC_ACTION` `lparam` wskazuje na `MSLLHOOKSTRUCT`.
        let flags = unsafe { (*(lparam.0 as *const MSLLHOOKSTRUCT)).flags };
        if !HookOrigin::from_mouse_flags(flags).is_injected() {
            LAST_PHYSICAL.store(now_ms(), Ordering::SeqCst);
        }
    }
    // SAFETY: jw.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn run(ready: &mpsc::SyncSender<Result<u32, String>>) {
    let mut msg = MSG::default();
    // SAFETY: utworzenie kolejki komunikatów wątku przed zgłoszeniem gotowości.
    let _ = unsafe { PeekMessageW(&raw mut msg, None, WM_USER, WM_USER, PM_NOREMOVE) };
    // SAFETY: moduł bieżącego procesu; hooki LL nie wstrzykują DLL.
    let module = unsafe { GetModuleHandleW(None) }.ok().map(HINSTANCE::from);
    // SAFETY: hooki niskiego poziomu z procedurami w tym module.
    let hooks = unsafe {
        (
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_hook), module, 0),
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), module, 0),
        )
    };
    let (Ok(kb), Ok(mouse)) = hooks else {
        for h in [hooks.0.ok(), hooks.1.ok()].into_iter().flatten() {
            // SAFETY: zdjęcie hooka założonego przez ten wątek.
            let _ = unsafe { UnhookWindowsHookEx(h) };
        }
        let _ = ready.send(Err("SetWindowsHookExW".into()));
        return;
    };
    // SAFETY: identyfikator bieżącego wątku.
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));
    // SAFETY: standardowa pętla komunikatów; kończy ją `WM_QUIT` z `Drop`.
    unsafe {
        while GetMessageW(&raw mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
        let _ = UnhookWindowsHookEx(kb);
        let _ = UnhookWindowsHookEx(mouse);
    }
}

struct Running {
    thread_id: u32,
    join: Option<JoinHandle<()>>,
}

/// Monitor aktywności (wątek hooków startuje przy pierwszym użyciu).
#[derive(Default)]
pub(crate) struct ActivityMonitor {
    running: Mutex<Option<Running>>,
}

impl std::fmt::Debug for ActivityMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActivityMonitor").finish_non_exhaustive()
    }
}

impl ActivityMonitor {
    fn lock(&self) -> MutexGuard<'_, Option<Running>> {
        self.running.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Uruchamia hooki (idempotentne). Brak hooka = odmowa wejścia (nie wykrylibyśmy przejęcia).
    pub(crate) fn ensure(&self) -> Result<(), GuiError> {
        let mut guard = self.lock();
        if guard.is_some() {
            return Ok(());
        }
        let (tx, rx) = mpsc::sync_channel(1);
        let join = std::thread::Builder::new()
            .name("alfa-hook-aktywnosci".into())
            .spawn(move || run(&tx))
            .map_err(|e| GuiError::Platform(PlatformError::Io(format!("wątek hooka: {e}"))))?;
        let unavailable = |why: String| {
            GuiError::Platform(PlatformError::Unsupported(format!(
                "hook aktywności niedostępny ({why}) — bez niego nie wykryję, że przejmujesz mysz lub klawiaturę"
            )))
        };
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(thread_id)) => {
                *guard = Some(Running {
                    thread_id,
                    join: Some(join),
                });
                Ok(())
            }
            Ok(Err(why)) => Err(unavailable(why)),
            Err(_) => Err(unavailable("brak odpowiedzi wątku".into())),
        }
    }

    /// Ostatnie fizyczne wejście.
    pub(crate) fn last_physical(&self) -> Option<u64> {
        let v = LAST_PHYSICAL.load(Ordering::SeqCst);
        (v != 0).then_some(v)
    }
}

impl Drop for ActivityMonitor {
    fn drop(&mut self) {
        if let Some(mut r) = self.lock().take() {
            // SAFETY: komunikat do kolejki wątku hooków, bez wskaźników.
            let posted = unsafe { PostThreadMessageW(r.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
            if posted.is_ok()
                && let Some(join) = r.join.take()
            {
                let _ = join.join();
            }
        }
    }
}
