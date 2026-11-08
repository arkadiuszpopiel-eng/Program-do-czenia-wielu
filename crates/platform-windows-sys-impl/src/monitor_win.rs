//! Wątek monitora sygnałów: okno komunikatów (`HWND_MESSAGE`) z powiadomieniami sesji
//! (`WTSRegisterSessionNotification` → `WM_WTSSESSION_CHANGE`) i zasilania
//! (`RegisterPowerSettingNotification`: zasilacz, poziom baterii, oszczędzanie → `WM_POWERBROADCAST`)
//! oraz licznikiem `SetTimer` (bezczynność i pełny ekran nie mają powiadomień). Każde
//! powiadomienie i tyknięcie = próbka wszystkich portów. Rejestracje powiadomień są
//! najlepszym wysiłkiem — bez nich licznik i tak wykryje zmianę w jednym okresie.

#![allow(unsafe_code)]

use std::mem::size_of;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use platform_contract::PlatformError;
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Power::{
    HPOWERNOTIFY, RegisterPowerSettingNotification, UnregisterPowerSettingNotification,
};
use windows::Win32::System::RemoteDesktop::{
    NOTIFY_FOR_THIS_SESSION, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
};
use windows::Win32::System::SystemServices::{
    GUID_ACDC_POWER_SOURCE, GUID_BATTERY_PERCENTAGE_REMAINING, GUID_POWER_SAVING_STATUS,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DEVICE_NOTIFY_WINDOW_HANDLE, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, HWND_MESSAGE, KillTimer, MSG, PM_NOREMOVE, PeekMessageW, PostMessageW,
    PostThreadMessageW, RegisterClassExW, SetTimer, TranslateMessage, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_APP, WM_POWERBROADCAST, WM_QUIT, WM_TIMER, WM_USER, WM_WTSSESSION_CHANGE,
    WNDCLASSEXW,
};
use windows::core::w;

use crate::signals::Shared;

/// Komunikat „próbkuj teraz” (wysyłany z procedury okna po powiadomieniu).
const WM_SAMPLE: u32 = WM_APP + 0x51;
const TIMER_ID: usize = 1;

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_WTSSESSION_CHANGE || msg == WM_POWERBROADCAST {
        // SAFETY: komunikat do własnego okna, bez wskaźników.
        let _ = unsafe { PostMessageW(Some(hwnd), WM_SAMPLE, WPARAM(0), LPARAM(0)) };
        // `WM_POWERBROADCAST`: TRUE = obsłużone.
        return LRESULT(1);
    }
    // SAFETY: domyślna obsługa pozostałych komunikatów.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

struct Registrations {
    hwnd: HWND,
    session: bool,
    power: Vec<HPOWERNOTIFY>,
}

impl Registrations {
    fn create(poll_ms: u32) -> Result<Self, String> {
        // SAFETY: moduł bieżącego procesu.
        let module = unsafe { GetModuleHandleW(None) }.ok().map(HINSTANCE::from);
        let class = w!("AlfaSygnalySystemowe");
        let wc = WNDCLASSEXW {
            cbSize: u32::try_from(size_of::<WNDCLASSEXW>()).unwrap_or(80),
            lpfnWndProc: Some(wndproc),
            hInstance: module.unwrap_or_default(),
            lpszClassName: class,
            ..WNDCLASSEXW::default()
        };
        // SAFETY: klasa z procedurą w tym module; ponowna rejestracja (drugi monitor) zwraca 0 —
        // wtedy `CreateWindowExW` użyje istniejącej klasy z tą samą procedurą.
        let _ = unsafe { RegisterClassExW(&raw const wc) };
        // SAFETY: okno tylko-komunikatów bez rodzica widocznego, bez parametrów.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                module,
                None,
            )
        }
        .map_err(|e| format!("CreateWindowExW: {e}"))?;
        // SAFETY: okno należy do tego wątku.
        let session =
            unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) }.is_ok();
        let power = [
            GUID_ACDC_POWER_SOURCE,
            GUID_BATTERY_PERCENTAGE_REMAINING,
            GUID_POWER_SAVING_STATUS,
        ]
        .iter()
        .filter_map(|g| {
            // SAFETY: odbiorca = uchwyt okna (`DEVICE_NOTIFY_WINDOW_HANDLE`), GUID statyczny.
            unsafe {
                RegisterPowerSettingNotification(HANDLE(hwnd.0), g, DEVICE_NOTIFY_WINDOW_HANDLE)
            }
            .ok()
        })
        .collect();
        // SAFETY: licznik okna bez procedury (komunikaty `WM_TIMER` w kolejce wątku).
        let _ = unsafe { SetTimer(Some(hwnd), TIMER_ID, poll_ms, None) };
        Ok(Self {
            hwnd,
            session,
            power,
        })
    }
}

impl Drop for Registrations {
    fn drop(&mut self) {
        // SAFETY: zwalnianie rejestracji założonych w `create` na tym samym wątku.
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TIMER_ID);
            for h in self.power.drain(..) {
                let _ = UnregisterPowerSettingNotification(h);
            }
            if self.session {
                let _ = WTSUnRegisterSessionNotification(self.hwnd);
            }
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn run(shared: &Shared, poll_ms: u32, ready: &mpsc::SyncSender<Result<u32, String>>) {
    let mut msg = MSG::default();
    // SAFETY: utworzenie kolejki komunikatów wątku przed zgłoszeniem gotowości.
    let _ = unsafe { PeekMessageW(&raw mut msg, None, WM_USER, WM_USER, PM_NOREMOVE) };
    let regs = match Registrations::create(poll_ms) {
        Ok(r) => r,
        Err(e) => {
            let _ = ready.send(Err(e));
            return;
        }
    };
    // SAFETY: identyfikator bieżącego wątku.
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));
    shared.sample();
    // SAFETY: standardowa pętla komunikatów; kończy ją `WM_QUIT` z `Drop`.
    unsafe {
        while GetMessageW(&raw mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_TIMER || msg.message == WM_SAMPLE {
                shared.sample();
                continue;
            }
            let _ = TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
    }
    drop(regs);
}

/// Działający wątek monitora (zatrzymywany w `Drop`).
pub(crate) struct Monitor {
    thread_id: u32,
    join: Option<JoinHandle<()>>,
}

impl Monitor {
    pub(crate) fn start(shared: Arc<Shared>, poll: Duration) -> Result<Self, PlatformError> {
        let poll_ms = u32::try_from(poll.as_millis()).unwrap_or(u32::MAX).max(100);
        let (tx, rx) = mpsc::sync_channel(1);
        let join = std::thread::Builder::new()
            .name("alfa-sygnaly".into())
            .spawn(move || run(&shared, poll_ms, &tx))
            .map_err(|e| PlatformError::Io(format!("wątek monitora: {e}")))?;
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(thread_id)) => Ok(Self {
                thread_id,
                join: Some(join),
            }),
            Ok(Err(why)) => {
                let _ = join.join();
                Err(PlatformError::Unsupported(format!(
                    "monitor sygnałów: {why}"
                )))
            }
            Err(_) => Err(PlatformError::Io(
                "monitor sygnałów: brak odpowiedzi wątku".into(),
            )),
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        // SAFETY: komunikat do kolejki wątku monitora, bez wskaźników.
        let posted = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        if posted.is_ok()
            && let Some(join) = self.join.take()
        {
            let _ = join.join();
        }
    }
}
