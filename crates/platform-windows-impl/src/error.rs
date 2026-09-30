//! Mapowanie błędów systemu (Win32/HRESULT, `std::io`) na `PlatformError` z polskim opisem.
//!
//! Część przenośna (tabela kodów, `io::Error`) jest testowana na każdej platformie;
//! konwersja z `windows::core::Error` żyje w `crate::win`.

use std::io;
use std::path::Path;

use platform_contract::PlatformError;

/// `HRESULT_FROM_WIN32(ERROR_ACCESS_DENIED)`.
pub(crate) const HR_ACCESS_DENIED: u32 = 0x8007_0005;
/// `HRESULT_FROM_WIN32(ERROR_HOTKEY_ALREADY_REGISTERED)`.
pub(crate) const HR_HOTKEY_ALREADY_REGISTERED: u32 = 0x8007_0581;
/// `HRESULT_FROM_WIN32(ERROR_CANCELLED)`.
pub(crate) const HR_CANCELLED: u32 = 0x8007_04C7;

/// Polskie opisy najczęstszych kodów (HRESULT; kody Win32 w postaci `0x8007xxxx`).
const DESCRIPTIONS: [(u32, &str); 22] = [
    (0x8000_4005, "nieokreślony błąd"),
    (
        0x8000_4001,
        "funkcja niezaimplementowana w tej wersji systemu",
    ),
    (0x8004_0154, "klasa COM niezarejestrowana"),
    (0x8007_0002, "nie znaleziono pliku"),
    (0x8007_0003, "nie znaleziono ścieżki"),
    (HR_ACCESS_DENIED, "odmowa dostępu"),
    (0x8007_0006, "nieprawidłowy uchwyt"),
    (0x8007_0008, "za mało pamięci"),
    (0x8007_000E, "brak pamięci"),
    (0x8007_0011, "cel leży na innym woluminie"),
    (0x8007_0020, "plik jest używany przez inny proces"),
    (0x8007_0050, "plik już istnieje"),
    (0x8007_0057, "niepoprawny argument"),
    (0x8007_007B, "niepoprawna nazwa pliku lub katalogu"),
    (0x8007_0091, "katalog nie jest pusty"),
    (0x8007_00B7, "obiekt już istnieje"),
    (HR_CANCELLED, "operacja anulowana"),
    (0x8007_0578, "nieprawidłowy uchwyt okna"),
    (
        HR_HOTKEY_ALREADY_REGISTERED,
        "skrót jest już zarejestrowany w systemie",
    ),
    (0x8007_05AA, "brak zasobów systemowych"),
    (0x8007_05B4, "przekroczono limit czasu"),
    (0x887A_0002, "nie znaleziono obiektu (DXGI)"),
];

/// Polski opis kodu HRESULT, jeśli znany.
pub(crate) fn describe_hresult(code: u32) -> Option<&'static str> {
    DESCRIPTIONS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, text)| *text)
}

/// `HRESULT_FROM_WIN32`.
pub(crate) fn hresult_from_win32(code: u32) -> u32 {
    if code == 0 || code & 0x8000_0000 != 0 {
        code
    } else {
        (code & 0xFFFF) | 0x8007_0000
    }
}

/// Błąd Win32/COM → `PlatformError`: kod szesnastkowo + polski opis (lub komunikat systemu).
pub(crate) fn from_hresult(context: &str, code: u32, system_message: &str) -> PlatformError {
    let text = describe_hresult(code).map_or_else(
        || {
            let sys = system_message.trim();
            if sys.is_empty() {
                "nieznany błąd systemu".to_owned()
            } else {
                format!("błąd systemu: {sys}")
            }
        },
        str::to_owned,
    );
    let message = format!("{context}: {text} (0x{code:08X})");
    match code {
        HR_ACCESS_DENIED => PlatformError::PermissionDenied(message),
        HR_HOTKEY_ALREADY_REGISTERED => PlatformError::HotkeyConflict(message),
        _ => PlatformError::Io(message),
    }
}

/// Błąd `std::io` dotyczący ścieżki → `PlatformError` (ścieżka podana przez wywołującego).
pub(crate) fn from_io(err: &io::Error, path: &Path) -> PlatformError {
    match err.kind() {
        io::ErrorKind::NotFound => PlatformError::NotFound(path.to_path_buf()),
        io::ErrorKind::AlreadyExists => PlatformError::AlreadyExists(path.to_path_buf()),
        io::ErrorKind::PermissionDenied => {
            PlatformError::PermissionDenied(format!("{}: odmowa dostępu", path.display()))
        }
        _ => {
            let described = err
                .raw_os_error()
                .and_then(|raw| u32::try_from(raw).ok())
                .filter(|_| cfg!(windows))
                .and_then(|raw| describe_hresult(hresult_from_win32(raw)));
            match described {
                Some(text) => PlatformError::Io(format!("{}: {text}", path.display())),
                None => PlatformError::Io(format!("{}: {err}", path.display())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hresult_mapping_uses_polish_descriptions() {
        assert_eq!(hresult_from_win32(5), HR_ACCESS_DENIED);
        assert_eq!(hresult_from_win32(1409), HR_HOTKEY_ALREADY_REGISTERED);
        assert_eq!(hresult_from_win32(0x8000_4005), 0x8000_4005);
        assert_eq!(hresult_from_win32(0), 0);
        assert!(matches!(
            from_hresult("RegisterHotKey", HR_HOTKEY_ALREADY_REGISTERED, ""),
            PlatformError::HotkeyConflict(m) if m.contains("0x80070581")
        ));
        assert!(matches!(
            from_hresult("OpenProcess", HR_ACCESS_DENIED, "Access is denied."),
            PlatformError::PermissionDenied(m) if m.contains("odmowa dostępu")
        ));
        let unknown = from_hresult("X", 0x8765_4321, "Something failed.");
        assert_eq!(
            unknown,
            PlatformError::Io("X: błąd systemu: Something failed. (0x87654321)".into())
        );
        let bare = from_hresult("X", 0x8765_4321, " ");
        assert!(matches!(bare, PlatformError::Io(m) if m.contains("nieznany błąd")));
    }

    #[test]
    fn io_errors_keep_caller_path() {
        let p = Path::new("/x/y");
        let nf = io::Error::from(io::ErrorKind::NotFound);
        assert_eq!(from_io(&nf, p), PlatformError::NotFound(p.into()));
        let ae = io::Error::from(io::ErrorKind::AlreadyExists);
        assert_eq!(from_io(&ae, p), PlatformError::AlreadyExists(p.into()));
        let pd = io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(matches!(
            from_io(&pd, p),
            PlatformError::PermissionDenied(_)
        ));
        let other = io::Error::other("dysk pełny");
        assert!(matches!(from_io(&other, p), PlatformError::Io(m) if m.contains("dysk pełny")));
    }
}
