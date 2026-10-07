//! Wspólne narzędzia FFI (tylko Windows): uchwyty RAII, napisy UTF-16, błędy Win32, wątki COM.
//!
//! Model wątków (PLAN §3.2): wywołania COM wymagające STA (`IFileOperation`) i MTA (MMDevice)
//! idą na krótkotrwały, dedykowany wątek z własnym `CoInitializeEx` — nigdy na wątku wywołującego
//! (może to być wątek tokio albo wątek audio RT, gdzie COM jest zabroniony).

#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;

use platform_contract::PlatformError;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, COINIT_MULTITHREADED, CoInitializeEx,
    CoUninitialize,
};
use windows::core::{Error as WinError, PCWSTR};

use crate::error::from_hresult;

/// Uchwyt jądra zamykany w `Drop`.
#[derive(Debug)]
pub(crate) struct OwnedHandle(HANDLE);

// SAFETY: uchwyt obiektu jądra jest ważny w całym procesie, niezależnie od wątku; operacje na nim
// (Wait*, Terminate*, CloseHandle) są bezpieczne wątkowo po stronie jądra.
unsafe impl Send for OwnedHandle {}
// SAFETY: jw. — współdzielony odczyt wartości uchwytu nie wymaga synchronizacji.
unsafe impl Sync for OwnedHandle {}

impl OwnedHandle {
    /// Przejmuje uchwyt; `None` dla uchwytu pustego lub `INVALID_HANDLE_VALUE`.
    pub(crate) fn new(handle: HANDLE) -> Option<Self> {
        (!handle.is_invalid()).then_some(Self(handle))
    }

    /// Surowy uchwyt (własność zostaje w `OwnedHandle`).
    pub(crate) fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: uchwyt należy wyłącznie do nas (konstruktor przejmuje własność) i jest zamykany raz.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Napis UTF-16 zakończony zerem.
pub(crate) fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain(Some(0)).collect()
}

/// `PCWSTR` wskazujący na bufor z `wide` (bufor musi żyć dłużej niż użycie wskaźnika).
pub(crate) fn pcwstr(buffer: &[u16]) -> PCWSTR {
    PCWSTR(buffer.as_ptr())
}

/// Tekst z bufora UTF-16 do pierwszego zera (niepoprawne surogaty zastępowane).
pub(crate) fn from_wide(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

/// Kod HRESULT jako liczba bez znaku (do tabel i komunikatów).
pub(crate) fn hresult_bits(err: &WinError) -> u32 {
    u32::from_ne_bytes(err.code().0.to_ne_bytes())
}

/// Błąd windows-rs → `PlatformError` z kodem i polskim opisem.
pub(crate) fn win_error(context: &str, err: &WinError) -> PlatformError {
    from_hresult(context, hresult_bits(err), &err.message())
}

/// Ostatni błąd wątku (`GetLastError`) → `PlatformError`.
pub(crate) fn last_error(context: &str) -> PlatformError {
    win_error(context, &WinError::from_thread())
}

/// Błąd windows-rs → `io::Error` (kody Win32 zachowują rodzaj, np. `AlreadyExists`).
pub(crate) fn io_from_win(err: &WinError) -> io::Error {
    let bits = hresult_bits(err);
    if bits & 0xFFFF_0000 == 0x8007_0000 {
        io::Error::from_raw_os_error(i32::from(u16::try_from(bits & 0xFFFF).unwrap_or(0)))
    } else {
        io::Error::other(format!("0x{bits:08X}: {}", err.message()))
    }
}

/// Rodzaj apartamentu COM dla wątku roboczego.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Apartment {
    /// Jednowątkowy (wymagany przez `IFileOperation`).
    Sta,
    /// Wielowątkowy (MMDevice, przyszłe UIA).
    Mta,
}

/// Uruchamia `work` na nowym wątku z zainicjalizowanym COM i czeka na wynik.
pub(crate) fn run_in_apartment<R, F>(apartment: Apartment, work: F) -> Result<R, PlatformError>
where
    R: Send + 'static,
    F: FnOnce() -> Result<R, PlatformError> + Send + 'static,
{
    let handle = std::thread::Builder::new()
        .name(format!("alfa-com-{apartment:?}").to_lowercase())
        .spawn(move || {
            let flags = match apartment {
                Apartment::Sta => COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE,
                Apartment::Mta => COINIT_MULTITHREADED,
            };
            // SAFETY: nowy wątek, COM inicjalizowany raz; `CoUninitialize` tylko po sukcesie.
            let hr = unsafe { CoInitializeEx(None, flags) };
            if hr.is_err() {
                return Err(win_error("CoInitializeEx", &WinError::from_hresult(hr)));
            }
            let result = work();
            // SAFETY: para do udanego `CoInitializeEx` na tym samym wątku; obiekty COM z `work`
            // zostały już zwolnione (wyszły z zasięgu).
            unsafe { CoUninitialize() };
            result
        })
        .map_err(|e| PlatformError::Io(format!("nie można uruchomić wątku COM: {e}")))?;
    handle
        .join()
        .map_err(|_| PlatformError::Io("wątek COM zakończył się paniką".into()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_round_trip_and_errors() {
        let w = wide("Zażółć");
        assert_eq!(w.last(), Some(&0));
        assert_eq!(from_wide(&w), "Zażółć");
        let err = WinError::from_hresult(windows::core::HRESULT(0x8007_0005_u32 as i32));
        assert!(matches!(
            win_error("test", &err),
            PlatformError::PermissionDenied(_)
        ));
        assert_eq!(io_from_win(&err).kind(), io::ErrorKind::PermissionDenied);
        let value = run_in_apartment(Apartment::Mta, || Ok(7)).unwrap();
        assert_eq!(value, 7);
    }
}
