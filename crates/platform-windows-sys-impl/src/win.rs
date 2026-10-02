//! Wspólne narzędzia FFI (tylko Windows): uchwyt RAII, napisy UTF-16, błąd Win32 → `PlatformError`.

#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use platform_contract::PlatformError;
use windows::Win32::Foundation::{CloseHandle, E_ACCESSDENIED, HANDLE};
use windows::Win32::System::Threading::CreateEventW;
use windows::core::PCWSTR;

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

/// Zdarzenie z ręcznym zerowaniem (stan początkowy: niezasygnalizowane).
pub(crate) fn manual_event() -> Result<OwnedHandle, PlatformError> {
    // SAFETY: zdarzenie anonimowe bez atrybutów bezpieczeństwa.
    let h = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
        .map_err(|e| win_error("CreateEventW", &e))?;
    OwnedHandle::new(h).ok_or_else(|| PlatformError::Io("CreateEventW: pusty uchwyt".into()))
}

/// Ścieżka jako UTF-16 zakończone zerem.
pub(crate) fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}

/// Tekst z bufora UTF-16 do pierwszego zera.
pub(crate) fn from_wide(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

/// Błąd windows-rs → `PlatformError` (odmowa dostępu osobno).
pub(crate) fn win_error(context: &str, err: &windows::core::Error) -> PlatformError {
    if err.code() == E_ACCESSDENIED {
        PlatformError::PermissionDenied(format!("{context}: odmowa dostępu"))
    } else {
        PlatformError::Io(format!("{context}: {err}"))
    }
}
