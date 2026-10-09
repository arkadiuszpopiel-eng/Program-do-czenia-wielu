//! Tekst z wyjścia programów konsolowych. `cmd.exe` i wiele narzędzi pisze do potoku w stronie
//! kodowej OEM (CP852 na polskim Windows), nie w UTF-8 — bez przeliczenia „ą ę ł" trafiały dalej
//! jako znaki zastępcze. Poprawny UTF-8 przechodzi bez zmian (PowerShell z ustawionym kodowaniem,
//! nowsze narzędzia).

#![allow(unsafe_code)]

/// Wyjście procesu jako tekst: UTF-8, a gdy bajty nim nie są — strona kodowa OEM systemu.
pub(crate) fn decode_console(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => from_oem(bytes).unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned()),
    }
}

#[cfg(windows)]
fn from_oem(bytes: &[u8]) -> Option<String> {
    use windows::Win32::Globalization::{
        CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS, MultiByteToWideChar,
    };

    // Opakowanie windows-rs rzutuje długość na `i32` — większe wejście obsługuje ścieżka zapasowa.
    if bytes.is_empty() || i32::try_from(bytes.len()).is_err() {
        return None;
    }
    let flags = MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0);
    // SAFETY: `bytes` to poprawny wycinek na czas wywołania; bez bufora wyjściowego funkcja tylko
    // zwraca wymaganą liczbę jednostek UTF-16.
    let needed = unsafe { MultiByteToWideChar(CP_OEMCP, flags, bytes, None) };
    let needed = usize::try_from(needed).ok().filter(|n| *n > 0)?;
    let mut wide = vec![0u16; needed];
    // SAFETY: bufor `wide` ma dokładnie `needed` jednostek, tyle samo zgłoszono przy pomiarze;
    // funkcja nie zapisuje poza przekazany wycinek.
    let written = unsafe { MultiByteToWideChar(CP_OEMCP, flags, bytes, Some(&mut wide)) };
    let written = usize::try_from(written).ok().filter(|n| *n > 0)?;
    Some(String::from_utf16_lossy(wide.get(..written)?))
}

#[cfg(not(windows))]
fn from_oem(_bytes: &[u8]) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::decode_console;

    #[test]
    fn utf8_passes_through_and_empty_is_empty() {
        assert_eq!(decode_console("zażółć\r\n".as_bytes()), "zażółć\r\n");
        assert_eq!(decode_console(b""), "");
        assert_eq!(decode_console(b"ascii only"), "ascii only");
    }

    /// Bajty niebędące UTF-8 nie dają znaków zastępczych, gdy system ma dla nich znak w stronie OEM.
    #[cfg(windows)]
    #[test]
    fn non_utf8_bytes_are_decoded_with_the_oem_code_page() {
        // 0x82 jest w każdej stronie OEM literą (CP852/CP437/CP850: „é"), a nie poprawnym UTF-8.
        let text = decode_console(&[b'a', 0x82, b'b']);
        assert_eq!(text.chars().count(), 3, "{text:?}");
        assert!(!text.contains('\u{FFFD}'), "{text:?}");
        assert!(text.starts_with('a') && text.ends_with('b'));
    }
}
