//! Okna v2 (Win32): `EnumWindows` (kolejność Z), filtr widocznych i niezamaskowanych przez DWM,
//! właściciel/obraz/podniesienie procesu, monitory z DPI, fokus, położenie i stan — każda zmiana
//! po sprawdzeniu strażnikiem tuż przed wywołaniem; wywołania asynchroniczne (`ShowWindowAsync`,
//! `SWP_ASYNCWINDOWPOS`) nie blokują na zawieszonym oknie. Ochrona okna z procesów powiązanych
//! (WebView2 Alfy, okna-własności, treść UWP) i drzewa procesów z migawki przy każdym wyliczeniu
//! i każdej zmianie (przegląd #2, P2-01).

#![allow(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::mem::size_of;

use platform_contract::{
    DesktopWindow, GuiError, MonitorInfo, ScreenRect, TargetGuard, WindowId, WindowState,
    validate_bounds,
};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowTextLengthW,
    GetWindowTextW, IsIconic, IsWindow, IsWindowVisible, IsZoomed, MONITORINFOF_PRIMARY,
    SET_WINDOW_POS_FLAGS, SHOW_WINDOW_CMD, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE,
    SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOZORDER, SetForegroundWindow, SetWindowPos,
    ShowWindowAsync, WindowFromPoint,
};
use windows::core::BOOL;

use crate::links::{ProcessTree, window_target};
use crate::win::{
    frame_and_window_rect, from_wide, hwnd_of, id_of, last_error, process_elevated, rect_of,
    root_of, win_error,
};

unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` to wskaźnik na `Vec<HWND>` z `top_level`, żywy przez całe synchroniczne
    // `EnumWindows` (wywołanie zwrotne na tym samym wątku).
    let list = unsafe { &mut *(lparam.0 as *mut Vec<HWND>) };
    list.push(hwnd);
    BOOL(1)
}

unsafe extern "system" fn collect_monitor(
    m: HMONITOR,
    _: HDC,
    _: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    // SAFETY: jw. — wskaźnik na `Vec<HMONITOR>` z `monitors`.
    let list = unsafe { &mut *(lparam.0 as *mut Vec<HMONITOR>) };
    list.push(m);
    BOOL(1)
}

fn top_level() -> Vec<HWND> {
    let mut handles: Vec<HWND> = Vec::new();
    // SAFETY: wskaźnik na `handles` żyje przez całe synchroniczne `EnumWindows`.
    let _ = unsafe { EnumWindows(Some(collect), LPARAM(&raw mut handles as isize)) };
    handles
}

fn visible(hwnd: HWND) -> bool {
    let mut cloaked = 0u32;
    // SAFETY: odczyty atrybutów okna do zmiennych lokalnych.
    unsafe {
        let _ = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, (&raw mut cloaked).cast::<c_void>(), 4);
        IsWindowVisible(hwnd).as_bool() && cloaked == 0
    }
}

fn text_of(hwnd: HWND) -> (String, String) {
    // SAFETY: odczyt długości tytułu, tytułu i klasy do buforów o znanym rozmiarze.
    unsafe {
        let len = usize::try_from(GetWindowTextLengthW(hwnd)).unwrap_or(0);
        let mut title = vec![0u16; len + 1];
        let n = usize::try_from(GetWindowTextW(hwnd, &mut title)).unwrap_or(0);
        let mut class = vec![0u16; 256];
        let c = usize::try_from(GetClassNameW(hwnd, &mut class)).unwrap_or(0);
        (
            from_wide(&title[..n.min(title.len())]),
            from_wide(&class[..c.min(class.len())]),
        )
    }
}

fn monitor_handles() -> Vec<HMONITOR> {
    let mut list: Vec<HMONITOR> = Vec::new();
    // SAFETY: wskaźnik na `list` żyje przez całe synchroniczne wyliczanie.
    let _ = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&raw mut list as isize),
        )
    };
    list
}

/// Monitory (kolejność wyliczenia = indeks).
pub(crate) fn monitors() -> Vec<MonitorInfo> {
    monitor_handles()
        .into_iter()
        .enumerate()
        .map(|(i, m)| {
            let mut info = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let (mut dx, mut dy) = (96u32, 96u32);
            // SAFETY: `cbSize` ustawione; bufory wyjściowe lokalne.
            unsafe {
                let _ = GetMonitorInfoW(m, &raw mut info);
                let _ = GetDpiForMonitor(m, MDT_EFFECTIVE_DPI, &raw mut dx, &raw mut dy);
            }
            MonitorInfo {
                index: u32::try_from(i).unwrap_or(u32::MAX),
                rect: rect_of(info.rcMonitor),
                work_area: rect_of(info.rcWork),
                dpi: dx,
                primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
            }
        })
        .collect()
}

/// Kontekst jednego wyliczenia okien (migawka procesów, okno pierwszego planu, monitory).
struct Listing<'a> {
    guard: &'a TargetGuard,
    tree: ProcessTree,
    foreground: HWND,
    monitors: Vec<HMONITOR>,
    elevated: BTreeMap<u32, bool>,
}

fn describe(hwnd: HWND, z: u32, l: &mut Listing<'_>) -> DesktopWindow {
    let target = window_target(hwnd, &l.tree);
    let protected = target.protected(l.guard);
    let (pid, image) = (target.pid, target.image);
    let elevated = *l
        .elevated
        .entry(pid)
        .or_insert_with(|| process_elevated(pid));
    let (foreground, monitors) = (l.foreground, &l.monitors);
    let (title, class_name) = text_of(hwnd);
    // SAFETY: zapytania o stan okna, monitor i DPI.
    let (minimized, maximized, monitor, dpi) = unsafe {
        (
            IsIconic(hwnd).as_bool(),
            IsZoomed(hwnd).as_bool(),
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
            GetDpiForWindow(hwnd),
        )
    };
    let state = if minimized {
        WindowState::Minimized
    } else if maximized {
        WindowState::Maximized
    } else {
        WindowState::Normal
    };
    DesktopWindow {
        id: id_of(hwnd),
        title,
        class_name,
        pid,
        protected,
        image,
        rect: frame_and_window_rect(hwnd).0,
        monitor: monitors
            .iter()
            .position(|m| *m == monitor)
            .and_then(|p| u32::try_from(p).ok())
            .unwrap_or(0),
        dpi,
        state,
        focused: hwnd == foreground,
        z_order: z,
        elevated,
    }
}

/// Widoczne okna najwyższego poziomu w kolejności Z (także chronione — do maskowania).
pub(crate) fn windows(guard: &TargetGuard) -> Vec<DesktopWindow> {
    let mut listing = Listing {
        guard,
        tree: ProcessTree::snapshot(),
        // SAFETY: odczyt okna pierwszego planu.
        foreground: unsafe { GetForegroundWindow() },
        monitors: monitor_handles(),
        elevated: BTreeMap::new(),
    };
    top_level()
        .into_iter()
        .filter(|&h| visible(h) && !frame_and_window_rect(h).0.is_empty())
        .enumerate()
        .map(|(z, h)| describe(h, u32::try_from(z).unwrap_or(u32::MAX), &mut listing))
        .collect()
}

/// Okno na pierwszym planie.
pub(crate) fn foreground(guard: &TargetGuard) -> Option<DesktopWindow> {
    // SAFETY: odczyt okna pierwszego planu.
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_invalid() {
        return None;
    }
    let root = root_of(fg);
    windows(guard).into_iter().find(|w| w.id == id_of(root))
}

/// Okno najwyższego poziomu pod punktem.
pub(crate) fn window_at(x: i32, y: i32) -> Option<WindowId> {
    // SAFETY: zapytanie o okno pod punktem.
    let hwnd = unsafe { WindowFromPoint(POINT { x, y }) };
    (!hwnd.is_invalid()).then(|| id_of(root_of(hwnd)))
}

/// Sprawdza okno przed zmianą: istnieje, nie jest chronione.
fn checked(guard: &TargetGuard, id: WindowId, what: &str) -> Result<HWND, GuiError> {
    let hwnd = hwnd_of(id);
    // SAFETY: sprawdzenie, czy uchwyt wskazuje istniejące okno.
    if hwnd.is_invalid() || !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return Err(GuiError::ElementNotFound(format!("okno {}", id.0)));
    }
    let target = window_target(hwnd, &ProcessTree::snapshot());
    target.check(guard, what)?;
    Ok(target.root)
}

fn show(hwnd: HWND, cmd: SHOW_WINDOW_CMD) -> Result<(), GuiError> {
    // SAFETY: asynchroniczna zmiana stanu istniejącego okna (nie blokuje na zawieszonym oknie).
    if unsafe { ShowWindowAsync(hwnd, cmd) }.as_bool() {
        Ok(())
    } else {
        Err(last_error("ShowWindowAsync"))
    }
}

/// Fokus (z obejściem blokady pierwszego planu przez `AttachThreadInput`, cofanym od razu).
pub(crate) fn focus(guard: &TargetGuard, id: WindowId) -> Result<(), GuiError> {
    let hwnd = checked(guard, id, "fokus okna")?;
    // SAFETY: odczyt stanu, przywrócenie i ustawienie pierwszego planu istniejącego okna.
    unsafe {
        if IsIconic(hwnd).as_bool() {
            show(hwnd, SW_RESTORE)?;
        }
        if SetForegroundWindow(hwnd).as_bool() {
            return Ok(());
        }
        let current = GetForegroundWindow();
        let target =
            windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(current, None);
        let own = GetCurrentThreadId();
        let attached =
            target != 0 && target != own && AttachThreadInput(own, target, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let ok = SetForegroundWindow(hwnd).as_bool();
        if attached {
            let _ = AttachThreadInput(own, target, false);
        }
        if ok {
            Ok(())
        } else {
            Err(GuiError::TargetChanged {
                expected: id.0,
                actual: Some(id_of(GetForegroundWindow()).0),
            })
        }
    }
}

/// Położenie i rozmiar (ramka DWM → prostokąt okna z niewidocznymi krawędziami).
pub(crate) fn set_bounds(
    guard: &TargetGuard,
    id: WindowId,
    rect: ScreenRect,
) -> Result<(), GuiError> {
    validate_bounds(&rect, &monitors())?;
    let hwnd = checked(guard, id, "zmiana położenia okna")?;
    // SAFETY: odczyt stanu istniejącego okna.
    if unsafe { IsZoomed(hwnd) }.as_bool() || unsafe { IsIconic(hwnd) }.as_bool() {
        show(hwnd, SW_RESTORE)?;
    }
    let (frame, window) = frame_and_window_rect(hwnd);
    let (l, t) = (frame.left - window.left, frame.top - window.top);
    let (r, b) = (window.right - frame.right, window.bottom - frame.bottom);
    let flags: SET_WINDOW_POS_FLAGS = SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
    // SAFETY: zmiana położenia istniejącego okna, asynchronicznie (bez blokowania na zawieszonym).
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            rect.left - l,
            rect.top - t,
            rect.width() + l + r,
            rect.height() + t + b,
            flags,
        )
    }
    .map_err(|e| win_error("SetWindowPos", &e))
}

/// Stan okna.
pub(crate) fn set_state(
    guard: &TargetGuard,
    id: WindowId,
    state: WindowState,
) -> Result<(), GuiError> {
    let hwnd = checked(guard, id, "stan okna")?;
    show(
        hwnd,
        match state {
            WindowState::Minimized => SW_MINIMIZE,
            WindowState::Maximized => SW_MAXIMIZE,
            WindowState::Normal => SW_RESTORE,
        },
    )
}
