//! Tekst z wyjścia programów konsolowych. `cmd.exe` i wiele narzędzi pisze do potoku w stronie
//! kodowej OEM (CP852 na polskim Windows), nie w UTF-8 — bez przeliczenia „ą ę ł" trafiały dalej
//! jako znaki zastępcze. Poprawny UTF-8 przechodzi bez zmian (PowerShell z ustawionym kodowaniem,
//! nowsze narzędzia).

#![allow(unsafe_code)]

/// Wyjście procesu jako tekst: UTF-8, a gdy bajty nim nie są — strona kodowa OEM systemu.
///
/// Jeden zły bajt nie może zepsuć całości (wcześniej każdy błąd UTF-8 przełączał cały tekst na
/// OEM): wyjście ucięte limitem w środku znaku oraz tekst UTF-8 z pojedynczymi obcymi bajtami
/// zostają UTF-8, a tylko obce bajty są czytane jako OEM. Cały tekst idzie przez OEM dopiero
/// wtedy, gdy obcych bajtów jest więcej niż poprawnych znaków wielobajtowych — tak wygląda
/// wyjście `cmd.exe` i starszych narzędzi.
pub(crate) fn decode_console(bytes: &[u8]) -> String {
    let Err(error) = std::str::from_utf8(bytes) else {
        return String::from_utf8_lossy(bytes).into_owned();
    };
    let (mut multibyte, mut foreign) = (0usize, 0usize);
    let mut truncated_tail = 0usize;
    for chunk in bytes.utf8_chunks() {
        multibyte += chunk.valid().chars().filter(|c| !c.is_ascii()).count();
        foreign += chunk.invalid().len();
    }
    // Niedokończony znak na samym końcu to skutek ucięcia strumienia, nie obce kodowanie.
    if error.error_len().is_none() {
        truncated_tail = bytes.len() - error.valid_up_to();
        foreign = foreign.saturating_sub(truncated_tail);
    }
    if foreign > multibyte {
        return from_oem(bytes).unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned());
    }
    let body = bytes.get(..bytes.len() - truncated_tail).unwrap_or(bytes);
    let mut text = String::with_capacity(body.len());
    for chunk in body.utf8_chunks() {
        text.push_str(chunk.valid());
        if !chunk.invalid().is_empty() {
            match from_oem(chunk.invalid()) {
                Some(decoded) => text.push_str(&decoded),
                None => text.push(char::REPLACEMENT_CHARACTER),
            }
        }
    }
    text
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

    /// Wyjście ucięte limitem w środku znaku wielobajtowego: reszta zostaje poprawnym UTF-8
    /// (wcześniej cały tekst szedł przez stronę OEM i każda polska litera stawała się krzaczkiem).
    #[test]
    fn output_cut_inside_a_character_stays_utf8() {
        let full = "zażółć gęślą jaźń — zażółć gęślą jaźń".as_bytes();
        for cut in 1..full.len() {
            let text = decode_console(&full[..cut]);
            assert!(
                "zażółć gęślą jaźń — zażółć gęślą jaźń".starts_with(&text),
                "cięcie {cut}: {text:?}"
            );
            assert!(
                full.len() - text.len() <= 3 + (full.len() - cut),
                "cięcie {cut}"
            );
        }
    }

    /// Tekst UTF-8 z jednym obcym bajtem: polskie litery zostają, obcy bajt nie psuje reszty.
    #[test]
    fn one_foreign_byte_does_not_turn_utf8_text_into_mojibake() {
        let mut bytes = "zażółć ".as_bytes().to_vec();
        bytes.push(0x82);
        bytes.extend_from_slice(" gęślą".as_bytes());
        let text = decode_console(&bytes);
        assert!(
            text.starts_with("zażółć ") && text.ends_with(" gęślą"),
            "{text:?}"
        );
        assert_eq!(
            text.chars().count(),
            "zażółć ".chars().count() + 1 + " gęślą".chars().count()
        );
    }

    /// Wyjście w stronie OEM (więcej obcych bajtów niż znaków UTF-8) nadal idzie w całości przez OEM.
    #[cfg(windows)]
    #[test]
    fn oem_text_is_still_decoded_as_a_whole() {
        // CP852: „zażółć" = 7A 61 BE A2 88 86 — żaden z bajtów ≥ 0x80 nie zaczyna poprawnego UTF-8.
        let text = decode_console(&[0x7A, 0x61, 0xBE, 0xA2, 0x88, 0x86]);
        assert_eq!(text.chars().count(), 6, "{text:?}");
        assert!(!text.contains('\u{FFFD}'), "{text:?}");
    }
}
