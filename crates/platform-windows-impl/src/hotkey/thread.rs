//! Dedykowany wątek skrótów: kolejka komunikatów, `RegisterHotKey` (skojarzone z wątkiem),
//! hook `WH_KEYBOARD_LL` (puszczenie klawisza < 10 ms) i zapasowe odpytywanie `GetAsyncKeyState`
//! co 15 ms (hook nie widzi klawiszy, gdy na pierwszym planie jest okno administratora — UIPI).
//!
//! Wątek nie robi nic poza obsługą komunikatów: callback hooka musi wracać natychmiast, inaczej
//! system spowalnia całą klawiaturę (`LowLevelHooksTimeout`). Brak COM na tym wątku.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread::JoinHandle;
use std::time::Duration;

use platform_contract::{HotkeyEvent, HotkeyId, PlatformError};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, KillTimer, MSG, PM_NOREMOVE,
    PeekMessageW, PostThreadMessageW, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx,
    WH_KEYBOARD_LL, WM_APP, WM_HOTKEY, WM_KEYUP, WM_QUIT, WM_SYSKEYUP, WM_TIMER, WM_USER,
};

use super::EventQueue;
use super::keys::{MOD_NOREPEAT, NativeHotkey, held_groups, releases_modifier};
use crate::win::win_error;

const WM_REQUEST: u32 = WM_APP + 1;
const RELEASE_POLL_MS: u32 = 15;
const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

enum Request {
    Register {
        id: u32,
        native: NativeHotkey,
        reply: SyncSender<Result<(), PlatformError>>,
    },
    Unregister {
        id: u32,
        reply: SyncSender<Result<(), PlatformError>>,
    },
}

/// Uchwyt wątku skrótów; `Drop` kończy pętlę (`WM_QUIT`) i czeka na wątek.
#[derive(Debug)]
pub(crate) struct HotkeyThread {
    thread_id: u32,
    requests: Sender<Request>,
    join: Option<JoinHandle<()>>,
}

struct Held {
    id: u32,
    native: NativeHotkey,
}

struct ThreadState {
    events: Arc<EventQueue>,
    registered: BTreeMap<u32, NativeHotkey>,
    held: Vec<Held>,
    timer: usize,
}

thread_local! {
    static STATE: RefCell<Option<ThreadState>> = const { RefCell::new(None) };
}

fn post(thread_id: u32, message: u32) -> Result<(), PlatformError> {
    // SAFETY: wysyłka komunikatu do kolejki wątku (bez wskaźników w parametrach).
    unsafe { PostThreadMessageW(thread_id, message, WPARAM(0), LPARAM(0)) }
        .map_err(|e| win_error("PostThreadMessageW", &e))
}

impl HotkeyThread {
    /// Startuje wątek i czeka, aż utworzy kolejkę komunikatów i hook.
    pub(crate) fn start(events: Arc<EventQueue>) -> Result<Self, PlatformError> {
        let (requests, inbox) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let join = std::thread::Builder::new()
            .name("alfa-hotkeys".into())
            .spawn(move || run(&inbox, events, &ready_tx))
            .map_err(|e| PlatformError::Io(format!("nie można uruchomić wątku skrótów: {e}")))?;
        let thread_id = ready_rx
            .recv_timeout(REPLY_TIMEOUT)
            .map_err(|_| PlatformError::Io("wątek skrótów nie odpowiedział".into()))?;
        Ok(Self {
            thread_id,
            requests,
            join: Some(join),
        })
    }

    fn call(
        &self,
        make: impl FnOnce(SyncSender<Result<(), PlatformError>>) -> Request,
    ) -> Result<(), PlatformError> {
        let (reply, answer) = mpsc::sync_channel(1);
        self.requests
            .send(make(reply))
            .map_err(|_| PlatformError::Io("wątek skrótów zakończony".into()))?;
        post(self.thread_id, WM_REQUEST)?;
        answer
            .recv_timeout(REPLY_TIMEOUT)
            .map_err(|_| PlatformError::Io("wątek skrótów nie odpowiedział".into()))?
    }

    /// Rejestruje skrót na wątku skrótów.
    pub(crate) fn register(&self, id: u32, native: NativeHotkey) -> Result<(), PlatformError> {
        self.call(|reply| Request::Register { id, native, reply })
    }

    /// Wyrejestrowuje skrót.
    pub(crate) fn unregister(&self, id: u32) -> Result<(), PlatformError> {
        self.call(|reply| Request::Unregister { id, reply })
    }
}

impl Drop for HotkeyThread {
    fn drop(&mut self) {
        if post(self.thread_id, WM_QUIT).is_ok()
            && let Some(join) = self.join.take()
        {
            let _ = join.join();
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut ThreadState) -> R) -> Option<R> {
    STATE.with(|cell| cell.try_borrow_mut().ok()?.as_mut().map(f))
}

fn release(state: &mut ThreadState, index: usize) {
    let held = state.held.swap_remove(index);
    state.events.push(HotkeyEvent {
        id: HotkeyId(held.id),
        pressed: false,
    });
    if state.held.is_empty() && state.timer != 0 {
        // SAFETY: licznik wątku utworzony przez `SetTimer(None, …)` na tym wątku.
        let _ = unsafe { KillTimer(None, state.timer) };
        state.timer = 0;
    }
}

/// Hook klawiatury niskiego poziomu: tylko puszczenia klawiszy, bez blokowania wejścia.
unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let is_up = matches!(u32::try_from(wparam.0), Ok(WM_KEYUP | WM_SYSKEYUP));
    if code == HC_ACTION as i32 && is_up && lparam.0 != 0 {
        // SAFETY: dla `HC_ACTION` system przekazuje w `lparam` wskaźnik na `KBDLLHOOKSTRUCT`.
        let vk = unsafe { (*(lparam.0 as *const KBDLLHOOKSTRUCT)).vkCode };
        with_state(|state| {
            while let Some(index) = state
                .held
                .iter()
                .position(|h| h.native.vk == vk || releases_modifier(vk, h.native.modifiers))
            {
                release(state, index);
            }
        });
    }
    // SAFETY: przekazanie zdarzenia dalej w łańcuchu hooków (wymóg API).
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

fn is_down(vk: u32) -> bool {
    // SAFETY: odczyt stanu klawisza (bez skutków ubocznych).
    i32::try_from(vk).is_ok_and(|vk| unsafe { GetAsyncKeyState(vk) } < 0)
}

fn on_hotkey(id: u32) {
    with_state(|state| {
        let Some(native) = state.registered.get(&id).copied() else {
            return;
        };
        state.events.push(HotkeyEvent {
            id: HotkeyId(id),
            pressed: true,
        });
        if !state.held.iter().any(|h| h.id == id) {
            state.held.push(Held { id, native });
        }
        if state.timer == 0 {
            // SAFETY: licznik wątku (bez okna i procedury) — przychodzi jako `WM_TIMER`.
            state.timer = unsafe { SetTimer(None, 0, RELEASE_POLL_MS, None) };
        }
    });
}

fn on_timer() {
    with_state(|state| {
        while let Some(index) = state.held.iter().position(|h| {
            !held_groups(&h.native)
                .iter()
                .all(|group| group.iter().any(|&vk| is_down(vk)))
        }) {
            release(state, index);
        }
    });
}

fn on_request(request: Request) {
    match request {
        Request::Register { id, native, reply } => {
            let modifiers = HOT_KEY_MODIFIERS(native.modifiers | MOD_NOREPEAT);
            // SAFETY: skrót skojarzony z bieżącym wątkiem (brak okna); `id` < 0xC000.
            let result = unsafe { RegisterHotKey(None, id as i32, modifiers, native.vk) }
                .map_err(|e| win_error("RegisterHotKey", &e));
            if result.is_ok() {
                with_state(|state| state.registered.insert(id, native));
            }
            let _ = reply.send(result);
        }
        Request::Unregister { id, reply } => {
            // SAFETY: wyrejestrowanie skrótu tego wątku.
            let result = unsafe { UnregisterHotKey(None, id as i32) }
                .map_err(|e| win_error("UnregisterHotKey", &e));
            with_state(|state| {
                state.registered.remove(&id);
                state.held.retain(|h| h.id != id);
            });
            let _ = reply.send(result);
        }
    }
}

fn run(inbox: &Receiver<Request>, events: Arc<EventQueue>, ready: &SyncSender<u32>) {
    let mut msg = MSG::default();
    // SAFETY: wymusza utworzenie kolejki komunikatów wątku przed zgłoszeniem gotowości.
    let _ = unsafe { PeekMessageW(&raw mut msg, None, WM_USER, WM_USER, PM_NOREMOVE) };
    STATE.with(|cell| {
        *cell.borrow_mut() = Some(ThreadState {
            events,
            registered: BTreeMap::new(),
            held: Vec::new(),
            timer: 0,
        });
    });
    // SAFETY: moduł bieżącego procesu; hook globalny w kodzie tego procesu (LL nie wstrzykuje DLL).
    let module = unsafe { GetModuleHandleW(None) }.ok().map(HINSTANCE::from);
    // SAFETY: jw.; brak hooka nie jest fatalny — zostaje odpytywanie `GetAsyncKeyState`.
    let hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), module, 0) }.ok();
    // SAFETY: odczyt identyfikatora bieżącego wątku.
    let _ = ready.send(unsafe { GetCurrentThreadId() });
    loop {
        // SAFETY: pętla komunikatów bieżącego wątku; `msg` to poprawny bufor.
        let got = unsafe { GetMessageW(&raw mut msg, None, 0, 0) };
        if got.0 <= 0 {
            break;
        }
        match msg.message {
            WM_REQUEST => {
                while let Ok(request) = inbox.try_recv() {
                    on_request(request);
                }
            }
            WM_HOTKEY => on_hotkey(u32::try_from(msg.wParam.0).unwrap_or(u32::MAX)),
            WM_TIMER => on_timer(),
            _ => {}
        }
    }
    let ids: Vec<u32> = with_state(|s| s.registered.keys().copied().collect()).unwrap_or_default();
    for id in ids {
        // SAFETY: sprzątanie skrótów tego wątku przed jego końcem.
        let _ = unsafe { UnregisterHotKey(None, id as i32) };
    }
    if let Some(hook) = hook {
        // SAFETY: hook zainstalowany przez ten wątek.
        let _ = unsafe { UnhookWindowsHookEx(hook) };
    }
    STATE.with(|cell| cell.borrow_mut().take());
}
