//! Zapytania Windows (bez stanu, z dowolnego wątku): bezczynność, zasilanie, tryb pełnego ekranu,
//! stan sesji.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::mem::size_of;

use platform_contract::{
    FullscreenProbe, NotificationState, PlatformError, PowerSnapshot, SessionState,
};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::RemoteDesktop::{
    WTS_CURRENT_SESSION, WTSFreeMemory, WTSINFOEXW, WTSQuerySessionInformationW, WTSSessionInfoEx,
};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Shell::SHQueryUserNotificationState;
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetClassNameW, GetDesktopWindow, GetForegroundWindow, GetShellWindow,
    GetWindowLongW, GetWindowRect, GetWindowThreadProcessId, IsIconic, WS_CAPTION,
};
use windows::core::PWSTR;

use crate::win::{OwnedHandle, from_wide, win_error};

/// Klasy okien powłoki, które pokrywają monitor, a nie są grą (pulpit, pasek zadań).
const SHELL_CLASSES: [&str; 4] = [
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
];

/// Milisekundy od ostatniego wejścia: `GetTickCount64` (dolne 32 bity) − `dwTime` modulo 2³²
/// (`dwTime` to `GetTickCount` — przekręca się co 49,7 dnia).
pub(crate) fn idle_ms() -> Result<u64, PlatformError> {
    let mut info = LASTINPUTINFO {
        cbSize: u32::try_from(size_of::<LASTINPUTINFO>()).unwrap_or(8),
        dwTime: 0,
    };
    // SAFETY: wskaźnik na poprawnie zainicjowaną strukturę z `cbSize`.
    if !unsafe { GetLastInputInfo(&raw mut info) }.as_bool() {
        return Err(PlatformError::Io(
            "GetLastInputInfo nie powiodło się".into(),
        ));
    }
    // SAFETY: bez argumentów.
    let now = unsafe { GetTickCount64() };
    let low = u32::try_from(now & u64::from(u32::MAX)).unwrap_or(0);
    Ok(u64::from(low.wrapping_sub(info.dwTime)))
}

/// Stan zasilania z `GetSystemPowerStatus`.
pub(crate) fn power() -> Result<PowerSnapshot, PlatformError> {
    let mut s = SYSTEM_POWER_STATUS::default();
    // SAFETY: wskaźnik na strukturę wyjściową.
    unsafe { GetSystemPowerStatus(&raw mut s) }
        .map_err(|e| win_error("GetSystemPowerStatus", &e))?;
    Ok(PowerSnapshot::from_system_power_status(
        s.ACLineStatus,
        s.BatteryFlag,
        s.BatteryLifePercent,
        s.SystemStatusFlag,
        s.BatteryLifeTime,
    ))
}

fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 64];
    // SAFETY: bufor wyjściowy o znanej długości.
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    usize::try_from(n).map_or_else(|_| String::new(), |n| from_wide(&buf[..n.min(buf.len())]))
}

fn image_name(pid: u32) -> Option<String> {
    // SAFETY: tylko zapytanie o obraz; uchwyt zamykany przez `OwnedHandle`.
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let h = OwnedHandle::new(h)?;
    let mut buf = vec![0u16; 1024];
    let mut size = u32::try_from(buf.len()).unwrap_or(1024);
    // SAFETY: bufor wyjściowy `size` znaków, uchwyt ważny.
    unsafe {
        QueryFullProcessImageNameW(
            h.raw(),
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &raw mut size,
        )
    }
    .ok()?;
    let full = from_wide(&buf);
    full.rsplit(['\\', '/']).next().map(str::to_owned)
}

fn covers(window: &RECT, monitor: &RECT) -> bool {
    window.left <= monitor.left
        && window.top <= monitor.top
        && window.right >= monitor.right
        && window.bottom >= monitor.bottom
}

/// Okno pierwszego planu pokrywa monitor (bez ramki, nie zminimalizowane, nie pulpit/pasek
/// zadań, nie okno tego procesu); zwraca też nazwę obrazu procesu.
fn foreground_fullscreen(own_pid: u32) -> (bool, Option<String>) {
    // SAFETY: zapytania o okna bez wskaźników wejściowych.
    let (hwnd, desktop, shell) =
        unsafe { (GetForegroundWindow(), GetDesktopWindow(), GetShellWindow()) };
    if hwnd.is_invalid() || hwnd == desktop || hwnd == shell {
        return (false, None);
    }
    if SHELL_CLASSES.contains(&class_name(hwnd).as_str()) {
        return (false, None);
    }
    let mut pid = 0u32;
    // SAFETY: wskaźnik na zmienną wyjściową.
    let _ = unsafe { GetWindowThreadProcessId(hwnd, Some(&raw mut pid)) };
    if pid == own_pid {
        return (false, None);
    }
    // SAFETY: zapytania o okno; struktury wyjściowe z `cbSize`.
    let covered = unsafe {
        let style = u32::from_ne_bytes(GetWindowLongW(hwnd, GWL_STYLE).to_ne_bytes());
        let captioned = style & WS_CAPTION.0 == WS_CAPTION.0;
        let mut rect = RECT::default();
        let mut info = MONITORINFO {
            cbSize: u32::try_from(size_of::<MONITORINFO>()).unwrap_or(40),
            ..MONITORINFO::default()
        };
        !IsIconic(hwnd).as_bool()
            && !captioned
            && GetWindowRect(hwnd, &raw mut rect).is_ok()
            && GetMonitorInfoW(
                MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
                &raw mut info,
            )
            .as_bool()
            && covers(&rect, &info.rcMonitor)
    };
    (covered, covered.then(|| image_name(pid)).flatten())
}

/// Próbka trybu pełnego ekranu (błąd `SHQueryUserNotificationState` → stan nieznany).
pub(crate) fn fullscreen(own_pid: u32) -> Result<FullscreenProbe, PlatformError> {
    // SAFETY: bez argumentów.
    let notification = unsafe { SHQueryUserNotificationState() }
        .map_or(NotificationState::Unknown, |s| {
            NotificationState::from_raw(s.0)
        });
    let (foreground_fullscreen, foreground_image) = foreground_fullscreen(own_pid);
    Ok(FullscreenProbe {
        notification,
        foreground_fullscreen,
        foreground_image,
    })
}

/// Stan sesji procesu: `WTSSessionInfoEx` (blokada z `SessionFlags`, rozłączenie ze `SessionState`).
pub(crate) fn session() -> Result<SessionState, PlatformError> {
    let mut buf = PWSTR::null();
    let mut bytes = 0u32;
    // SAFETY: bieżący serwer (`None`) i sesja; bufor zwalniany `WTSFreeMemory`.
    unsafe {
        WTSQuerySessionInformationW(
            None,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &raw mut buf,
            &raw mut bytes,
        )
    }
    .map_err(|e| win_error("WTSQuerySessionInformationW", &e))?;
    if buf.is_null() {
        return Ok(SessionState::Unknown);
    }
    let big_enough = usize::try_from(bytes).is_ok_and(|b| b >= size_of::<WTSINFOEXW>());
    let state = if big_enough {
        // SAFETY: bufor od WTS ma co najmniej `size_of::<WTSINFOEXW>()` bajtów i wyrównanie
        // alokatora systemowego; unia czytana tylko dla poziomu 1.
        unsafe {
            let info = &*buf.0.cast_const().cast::<WTSINFOEXW>();
            if info.Level == 1 {
                let l1 = info.Data.WTSInfoExLevel1;
                SessionState::from_wts(l1.SessionState.0, l1.SessionFlags)
            } else {
                SessionState::Unknown
            }
        }
    } else {
        SessionState::Unknown
    };
    // SAFETY: bufor przydzielony przez `WTSQuerySessionInformationW`, zwalniany raz.
    unsafe { WTSFreeMemory(buf.0.cast::<c_void>()) };
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_cover() {
        let m = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        assert!(covers(&m, &m));
        let w = RECT { right: 1919, ..m };
        assert!(!covers(&w, &m));
    }
}
