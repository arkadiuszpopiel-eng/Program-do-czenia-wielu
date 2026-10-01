//! Próbka wejścia dla okna Broker-UI: `GetCurrentInputMessageSource` + ostatnia obserwacja
//! hooków `WH_KEYBOARD_LL`/`WH_MOUSE_LL` (flagi `LL*HF_INJECTED`, `LL*HF_LOWER_IL_INJECTED`) →
//! `input_is_injected` (fail-closed) oraz test zasłonięcia przez okna wyżej w kolejności Z.
//! Hooki działają na wątku okna (on pompuje komunikaty) i tylko zapisują obserwację.

#![allow(unsafe_code)]

use std::cell::Cell;
use std::ffi::c_void;

use platform_contract::{
    HookObservation, HookOrigin, InputDevice, InputSample, MessageOrigin, input_is_injected,
};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::UI::Input::{GetCurrentInputMessageSource, INPUT_MESSAGE_SOURCE};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GW_HWNDPREV, GetWindow, GetWindowRect, GetWindowThreadProcessId, HC_ACTION,
    HHOOK, IsWindowVisible, KBDLLHOOKSTRUCT, MSLLHOOKSTRUCT, SetWindowsHookExW,
    UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN,
    WM_RBUTTONUP,
};

use super::now_ms;

thread_local! {
    static LAST_KEY: Cell<Option<HookObservation>> = const { Cell::new(None) };
    static LAST_MOUSE: Cell<Option<HookObservation>> = const { Cell::new(None) };
}

fn observe(cell: &'static std::thread::LocalKey<Cell<Option<HookObservation>>>, o: HookOrigin) {
    cell.set(Some(HookObservation {
        origin: o,
        at_ms: now_ms(),
    }));
}

unsafe extern "system" fn key_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && lparam.0 != 0 {
        // SAFETY: dla `HC_ACTION` `lparam` wskazuje na `KBDLLHOOKSTRUCT`.
        let flags = unsafe { (*(lparam.0 as *const KBDLLHOOKSTRUCT)).flags.0 };
        observe(&LAST_KEY, HookOrigin::from_keyboard_flags(flags));
    }
    // SAFETY: przekazanie dalej w łańcuchu hooków (wymóg API; hook nie blokuje wejścia).
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

unsafe extern "system" fn mouse_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let button = matches!(
        u32::try_from(wparam.0),
        Ok(WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP)
    );
    if code == HC_ACTION as i32 && button && lparam.0 != 0 {
        // SAFETY: dla `HC_ACTION` `lparam` wskazuje na `MSLLHOOKSTRUCT`.
        let flags = unsafe { (*(lparam.0 as *const MSLLHOOKSTRUCT)).flags };
        observe(&LAST_MOUSE, HookOrigin::from_mouse_flags(flags));
    }
    // SAFETY: jw.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// Instaluje hooki na bieżącym wątku (brak hooka nie jest fatalny — decyzja i tak fail-closed).
pub(super) fn install_hooks(module: Option<HINSTANCE>) -> [Option<HHOOK>; 2] {
    // SAFETY: hooki niskiego poziomu w kodzie tego procesu (LL nie wstrzykuje DLL).
    unsafe {
        [
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_hook), module, 0).ok(),
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), module, 0).ok(),
        ]
    }
}

/// Zdejmuje hooki.
pub(super) fn remove_hooks(hooks: [Option<HHOOK>; 2]) {
    for h in hooks.into_iter().flatten() {
        // SAFETY: hook zainstalowany przez ten wątek.
        let _ = unsafe { UnhookWindowsHookEx(h) };
    }
}

/// Próbka bieżącego komunikatu wejścia (wołana w obsłudze `WM_COMMAND`/`WM_CLOSE`).
pub(super) fn sample() -> InputSample {
    let at_ms = now_ms();
    let mut src = INPUT_MESSAGE_SOURCE::default();
    // SAFETY: zapis do lokalnej struktury.
    let ok = unsafe { GetCurrentInputMessageSource(&raw mut src) }.is_ok();
    let id = |v: i32| u32::try_from(v).unwrap_or(0);
    let (device, origin) = if ok {
        let device = InputDevice::from_imdt(id(src.deviceType.0));
        (device, MessageOrigin::from_imo(id(src.originId.0)))
    } else {
        (InputDevice::Unknown, MessageOrigin::Unavailable)
    };
    let hook = match device {
        InputDevice::Keyboard => LAST_KEY.get(),
        InputDevice::Mouse | InputDevice::Touch | InputDevice::Pen => LAST_MOUSE.get(),
        InputDevice::Unknown => None,
    };
    InputSample {
        device,
        injected: input_is_injected(origin, hook, at_ms),
        at_ms,
    }
}

/// Prostokąt okna na ekranie.
fn rect(hwnd: HWND) -> Option<RECT> {
    let mut r = RECT::default();
    // SAFETY: zapis do lokalnej struktury.
    unsafe { GetWindowRect(hwnd, &raw mut r) }.ok().map(|()| r)
}

/// Czy widoczne, niezamaskowane okno innego procesu wyżej w kolejności Z nachodzi na nasze
/// (nakładka przekierowująca kliknięcie — clickjacking).
pub(super) fn occluded(hwnd: HWND) -> bool {
    let Some(mine) = rect(hwnd) else {
        return true;
    };
    // SAFETY: nawigacja po kolejności Z (uchwyty tylko do zapytań).
    let mut next = unsafe { GetWindow(hwnd, GW_HWNDPREV) };
    while let Ok(other) = next {
        let (mut pid, mut cloaked) = (0u32, 0u32);
        // SAFETY: zapytania o stan okna do zmiennych lokalnych; błąd DWM = niezamaskowane.
        let visible = unsafe {
            GetWindowThreadProcessId(other, Some(&raw mut pid));
            let _ =
                DwmGetWindowAttribute(other, DWMWA_CLOAKED, (&raw mut cloaked).cast::<c_void>(), 4);
            IsWindowVisible(other).as_bool()
        };
        let hit = rect(other).is_some_and(|r| {
            r.left < mine.right && mine.left < r.right && r.top < mine.bottom && mine.top < r.bottom
        });
        if visible && cloaked == 0 && pid != std::process::id() && hit {
            return true;
        }
        // SAFETY: jw.
        next = unsafe { GetWindow(other, GW_HWNDPREV) };
    }
    false
}
