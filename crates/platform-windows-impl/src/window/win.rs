//! Win32 okien: `EnumWindows` + filtry (widoczne, bez właściciela, nie narzędziowe, nie ukryte
//! przez DWM), dane procesu, monitora i DPI; fokus z obejściem blokady pierwszego planu.

#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::mem::size_of;

use platform_contract::{PlatformError, WindowId, WindowInfo};
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GW_OWNER, GWL_EXSTYLE, GWL_STYLE, GetForegroundWindow,
    GetShellWindow, GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
    MONITORINFOF_PRIMARY, SHOW_WINDOW_CMD, SW_MINIMIZE, SW_RESTORE, SetForegroundWindow,
    ShowWindowAsync, WS_CAPTION, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PWSTR};

use super::{Rect, WindowAction, WindowDetails, WindowGuard};
use crate::win::{OwnedHandle, from_wide, last_error};

fn hwnd_of(id: WindowId) -> HWND {
    HWND(usize::try_from(id.0).unwrap_or(0) as *mut c_void)
}

fn id_of(hwnd: HWND) -> WindowId {
    WindowId(hwnd.0 as usize as u64)
}

fn rect_of(r: RECT) -> Rect {
    Rect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    }
}

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` to wskaźnik na `Vec<HWND>` przekazany przez `enumerate`, żywy przez całe
    // `EnumWindows` (wywołanie zwrotne jest synchroniczne, na tym samym wątku).
    let list = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
    list.push(hwnd);
    BOOL(1)
}

fn is_candidate(hwnd: HWND) -> bool {
    // SAFETY: odczyty atrybutów okna; uchwyt pochodzi z `EnumWindows` (może już nie istnieć —
    // wtedy funkcje zwracają wartości zerowe).
    unsafe {
        let visible = IsWindowVisible(hwnd).as_bool();
        let owned = GetWindow(hwnd, GW_OWNER).is_ok_and(|o| !o.is_invalid());
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast::<c_void>(),
            size_of::<u32>() as u32,
        );
        visible
            && !owned
            && ex_style & WS_EX_TOOLWINDOW.0 == 0
            && cloaked == 0
            && GetWindowTextLengthW(hwnd) > 0
            && hwnd != GetShellWindow()
    }
}

fn title(hwnd: HWND) -> String {
    // SAFETY: odczyt długości i tytułu do bufora o znanym rozmiarze.
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    let mut buffer = vec![0u16; usize::try_from(len).unwrap_or(0) + 1];
    // SAFETY: jw.
    let written = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    from_wide(&buffer[..usize::try_from(written).unwrap_or(0)])
}

fn window_pid(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: `pid` to poprawny bufor wyjściowy.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
    pid
}

fn process_name(pid: u32) -> String {
    // SAFETY: minimalne prawo zapytania; uchwyt przejmuje `OwnedHandle`.
    let Ok(raw) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else {
        return String::new();
    };
    let Some(process) = OwnedHandle::new(raw) else {
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
    let full = from_wide(&buffer[..usize::try_from(len).unwrap_or(0)]);
    full.rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn frame_rect(hwnd: HWND) -> Rect {
    let mut r = RECT::default();
    // SAFETY: bufor wyjściowy o rozmiarze `RECT`; przy błędzie DWM używamy `GetWindowRect`.
    let dwm = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut r).cast::<c_void>(),
            size_of::<RECT>() as u32,
        )
    };
    if dwm.is_err() {
        // SAFETY: jw.
        let _ = unsafe { GetWindowRect(hwnd, &raw mut r) };
    }
    rect_of(r)
}

fn details(
    hwnd: HWND,
    foreground: HWND,
    guard: &WindowGuard,
    names: &mut BTreeMap<u32, String>,
) -> WindowDetails {
    let pid = window_pid(hwnd);
    let process = names
        .entry(pid)
        .or_insert_with(|| process_name(pid))
        .clone();
    let rect = frame_rect(hwnd);
    // SAFETY: monitor najbliższy oknu (zawsze istnieje przy MONITOR_DEFAULTTONEAREST).
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info.cbSize` ustawione; bufor wyjściowy.
    let _ = unsafe { GetMonitorInfoW(monitor, &raw mut info) };
    let monitor_rect = rect_of(info.rcMonitor);
    // SAFETY: odczyty stylu, stanu i DPI okna.
    let (style, minimized, dpi) = unsafe {
        (
            GetWindowLongPtrW(hwnd, GWL_STYLE) as u32,
            IsIconic(hwnd).as_bool(),
            GetDpiForWindow(hwnd),
        )
    };
    let captioned = style & WS_CAPTION.0 == WS_CAPTION.0;
    WindowDetails {
        info: WindowInfo {
            id: id_of(hwnd),
            title: title(hwnd),
            process: process.clone(),
            focused: hwnd == foreground,
            fullscreen: !minimized && !captioned && rect.covers(&monitor_rect),
        },
        pid,
        rect,
        monitor: monitor.0 as usize as u64,
        monitor_rect,
        primary_monitor: info.dwFlags & MONITORINFOF_PRIMARY != 0,
        dpi,
        minimized,
        protected: guard.is_protected(pid, &process),
    }
}

/// Widoczne okna najwyższego poziomu w kolejności Z (od góry).
pub(crate) fn enumerate(guard: &WindowGuard) -> Vec<WindowDetails> {
    let mut handles: Vec<HWND> = Vec::new();
    // SAFETY: wskaźnik na `handles` żyje przez całe synchroniczne `EnumWindows`.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&raw mut handles as isize)) };
    // SAFETY: odczyt okna pierwszego planu.
    let foreground = unsafe { GetForegroundWindow() };
    let mut names = BTreeMap::new();
    handles
        .into_iter()
        .filter(|&h| is_candidate(h))
        .map(|h| details(h, foreground, guard, &mut names))
        .collect()
}

/// PID i nazwa procesu właściciela okna.
pub(crate) fn owner(id: WindowId) -> Result<(u32, String), PlatformError> {
    let hwnd = hwnd_of(id);
    // SAFETY: sprawdzenie, czy uchwyt wskazuje istniejące okno.
    if hwnd.is_invalid() || !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return Err(PlatformError::UnknownResource(format!("okno {}", id.0)));
    }
    let pid = window_pid(hwnd);
    Ok((pid, process_name(pid)))
}

fn force_foreground(hwnd: HWND) -> bool {
    // SAFETY: operacje na istniejącym oknie; dołączenie kolejek wejścia jest odwracane zaraz po próbie.
    unsafe {
        if SetForegroundWindow(hwnd).as_bool() {
            return true;
        }
        let current = GetForegroundWindow();
        let target_thread = GetWindowThreadProcessId(current, None);
        let own_thread = GetCurrentThreadId();
        let attached = target_thread != 0
            && target_thread != own_thread
            && AttachThreadInput(own_thread, target_thread, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let ok = SetForegroundWindow(hwnd).as_bool();
        if attached {
            let _ = AttachThreadInput(own_thread, target_thread, false);
        }
        ok
    }
}

fn show_async(hwnd: HWND, command: SHOW_WINDOW_CMD) -> Result<(), PlatformError> {
    // SAFETY: asynchroniczna zmiana stanu istniejącego okna (nie blokuje na zawieszonym oknie).
    if unsafe { ShowWindowAsync(hwnd, command) }.as_bool() {
        Ok(())
    } else {
        Err(last_error("ShowWindowAsync"))
    }
}

/// Wykonuje akcję (wywołujący sprawdził już ochronę procesu).
pub(crate) fn apply(id: WindowId, action: WindowAction) -> Result<(), PlatformError> {
    let hwnd = hwnd_of(id);
    match action {
        WindowAction::Minimize => show_async(hwnd, SW_MINIMIZE),
        WindowAction::Restore => show_async(hwnd, SW_RESTORE),
        WindowAction::Focus => {
            // SAFETY: odczyt stanu i ewentualne przywrócenie istniejącego okna.
            if unsafe { IsIconic(hwnd) }.as_bool() {
                show_async(hwnd, SW_RESTORE)?;
            }
            if force_foreground(hwnd) {
                Ok(())
            } else {
                let err = last_error("SetForegroundWindow");
                Err(PlatformError::PermissionDenied(format!(
                    "system zablokował zmianę fokusu (ochrona przed kradzieżą fokusu): {err}"
                )))
            }
        }
    }
}
