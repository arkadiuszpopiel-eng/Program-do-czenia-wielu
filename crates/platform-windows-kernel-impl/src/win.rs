//! Wspólne narzędzia FFI (tylko Windows): uchwyty RAII, napisy UTF-16, błędy Win32. Kopia
//! podzbioru `platform-windows-impl/src/win.rs` (zakaz zależności `-impl` → cudzy `-impl`).

#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;

use platform_contract::PlatformError;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_and_errors() {
        let w = wide("Zażółć");
        assert_eq!(w.last(), Some(&0));
        let err = WinError::from_hresult(windows::core::HRESULT(0x8007_0005_u32 as i32));
        assert!(matches!(
            win_error("test", &err),
            PlatformError::PermissionDenied(_)
        ));
        assert_eq!(io_from_win(&err).kind(), io::ErrorKind::PermissionDenied);
    }
}
