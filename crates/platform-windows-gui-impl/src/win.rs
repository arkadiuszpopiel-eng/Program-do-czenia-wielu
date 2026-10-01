//! Wspólne narzędzia FFI (tylko Windows): uchwyty RAII, napisy UTF-16, błędy Win32, okno ↔
//! `WindowId`, obraz i podniesienie procesu (podstawa strażnika celów — fail-closed: proces,
//! którego obrazu nie da się odczytać, jest traktowany jak chroniony, a którego tokenu nie da się
//! otworzyć — jak podniesiony).

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::mem::size_of;
use std::time::{SystemTime, UNIX_EPOCH};

use platform_contract::{GuiError, PlatformError, ScreenRect, TargetWindow, WindowId};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{
    OpenProcess, OpenProcessToken, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetWindowRect, GetWindowThreadProcessId,
};
use windows::core::{Error as WinError, PWSTR};

/// Uchwyt jądra zamykany w `Drop`.
#[derive(Debug)]
pub(crate) struct OwnedHandle(HANDLE);

// SAFETY: uchwyt obiektu jądra jest ważny w całym procesie, niezależnie od wątku.
unsafe impl Send for OwnedHandle {}
// SAFETY: jw. — współdzielony odczyt wartości uchwytu nie wymaga synchronizacji.
unsafe impl Sync for OwnedHandle {}

impl OwnedHandle {
    /// Przejmuje uchwyt; `None` dla pustego lub `INVALID_HANDLE_VALUE`.
    pub(crate) fn new(handle: HANDLE) -> Option<Self> {
        (!handle.is_invalid()).then_some(Self(handle))
    }

    /// Surowy uchwyt (własność zostaje tutaj).
    pub(crate) fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: uchwyt należy wyłącznie do nas i jest zamykany raz.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Tekst z bufora UTF-16 do pierwszego zera.
pub(crate) fn from_wide(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

/// Błąd windows-rs → `GuiError` (odmowa dostępu osobno — zwykle UIPI/okno podniesione).
pub(crate) fn win_error(context: &str, err: &WinError) -> GuiError {
    let code = u32::from_ne_bytes(err.code().0.to_ne_bytes());
    let text = format!("{context}: 0x{code:08X} {}", err.message());
    if code == 0x8007_0005 {
        GuiError::Platform(PlatformError::PermissionDenied(text))
    } else {
        GuiError::Platform(PlatformError::Io(text))
    }
}

/// Ostatni błąd wątku.
pub(crate) fn last_error(context: &str) -> GuiError {
    win_error(context, &WinError::from_thread())
}

/// Chwila w ms od epoki UNIX (ta sama domena czasu co hook aktywności).
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// `WindowId` → `HWND`.
pub(crate) fn hwnd_of(id: WindowId) -> HWND {
    HWND(usize::try_from(id.0).unwrap_or(0) as *mut c_void)
}

/// `HWND` → `WindowId`.
pub(crate) fn id_of(hwnd: HWND) -> WindowId {
    WindowId(hwnd.0 as usize as u64)
}

/// `RECT` → `ScreenRect`.
pub(crate) fn rect_of(r: RECT) -> ScreenRect {
    ScreenRect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    }
}

/// Okno najwyższego poziomu zawierające `hwnd`.
pub(crate) fn root_of(hwnd: HWND) -> HWND {
    // SAFETY: zapytanie o przodka; dla nieistniejącego okna zwraca pusty uchwyt.
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if root.is_invalid() { hwnd } else { root }
}

/// PID właściciela okna (0 = nieznany).
pub(crate) fn window_pid(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: `pid` to poprawny bufor wyjściowy.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
    pid
}

fn open_query(pid: u32) -> Option<OwnedHandle> {
    // SAFETY: minimalne prawo zapytania; uchwyt przejmuje `OwnedHandle`.
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    OwnedHandle::new(raw)
}

/// Pełna ścieżka obrazu procesu (pusta = nieznany → strażnik traktuje jak chroniony).
pub(crate) fn process_image(pid: u32) -> String {
    let Some(process) = (pid != 0).then(|| open_query(pid)).flatten() else {
        return String::new();
    };
    let mut buffer = vec![0u16; 1024];
    let mut len = u32::try_from(buffer.len()).unwrap_or(0);
    // SAFETY: bufor i jego rozmiar w znakach; `len` dostaje długość wyniku.
    let ok = unsafe {
        QueryFullProcessImageNameW(
            process.raw(),
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut len,
        )
    };
    if ok.is_err() {
        return String::new();
    }
    from_wide(&buffer[..usize::try_from(len).unwrap_or(0).min(buffer.len())])
}

/// Czy proces jest podniesiony (`TokenElevation`); brak dostępu do tokenu = podniesiony.
pub(crate) fn process_elevated(pid: u32) -> bool {
    let Some(process) = open_query(pid) else {
        return true;
    };
    let mut token = HANDLE::default();
    // SAFETY: token procesu tylko do zapytań; uchwyt przejmuje `OwnedHandle`.
    if unsafe { OpenProcessToken(process.raw(), TOKEN_QUERY, &raw mut token) }.is_err() {
        return true;
    }
    let Some(token) = OwnedHandle::new(token) else {
        return true;
    };
    let mut elevation = TOKEN_ELEVATION::default();
    let mut len = 0u32;
    // SAFETY: bufor wyjściowy o rozmiarze `TOKEN_ELEVATION`.
    let ok = unsafe {
        GetTokenInformation(
            token.raw(),
            TokenElevation,
            Some((&raw mut elevation).cast::<c_void>()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &raw mut len,
        )
    };
    ok.is_err() || elevation.TokenIsElevated != 0
}

/// Cel wejścia dla okna (właściciel, obraz, podniesienie).
pub(crate) fn target_of(hwnd: HWND) -> Option<TargetWindow> {
    if hwnd.is_invalid() {
        return None;
    }
    let root = root_of(hwnd);
    let pid = window_pid(root);
    Some(TargetWindow {
        id: id_of(root),
        pid,
        image: process_image(pid),
        elevated: process_elevated(pid),
    })
}

/// Prostokąt ramki okna (DWM, bez niewidocznych krawędzi) i prostokąt okna (`GetWindowRect`).
pub(crate) fn frame_and_window_rect(hwnd: HWND) -> (ScreenRect, ScreenRect) {
    let mut window = RECT::default();
    // SAFETY: bufor wyjściowy `RECT`.
    let _ = unsafe { GetWindowRect(hwnd, &raw mut window) };
    let mut frame = RECT::default();
    // SAFETY: bufor wyjściowy o rozmiarze `RECT`; przy błędzie DWM używamy `GetWindowRect`.
    let dwm = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut frame).cast::<c_void>(),
            size_of::<RECT>() as u32,
        )
    };
    if dwm.is_err() {
        frame = window;
    }
    (rect_of(frame), rect_of(window))
}
