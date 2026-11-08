//! Przygotowanie surowych bajtów przed `lopdf` (te same długości — przesunięcia xref się nie
//! zmieniają). Ukrywamy przed `lopdf` dwie ścieżki bez naszych limitów:
//! - `/Encrypt` w trailerze → `/AlfaEnc`: `lopdf` nie wejdzie w ścieżkę dokumentów zaszyfrowanych
//!   (ta pomija filtr wczytywania i sama parsuje strumienie obiektów); odszyfrowujemy sami;
//! - pośrednie `/Length N G R` → `/AlfaLn`: `lopdf` rozwiązywałby je w trakcie czytania, a dla
//!   obiektu ze strumienia obiektów parsowałby cały ten strumień bez limitów (kopie obiektów).
//!   Treść takich strumieni dokładamy sami po wczytaniu.
//!
//! Przy okazji sumujemy bezpośrednie `/Length`: w zdrowym pliku dane strumieni się nie nakładają,
//! więc suma nie przekracza rozmiaru pliku — większa oznacza strumienie zagnieżdżone w sobie
//! (każdy kopiowany osobno), czyli plik wrogi.

/// Ukryta nazwa klucza `Encrypt` (ta sama długość co `/Encrypt`).
pub(super) const HIDDEN_ENCRYPT: &[u8] = b"AlfaEnc";
/// Ukryta nazwa klucza `Length` (ta sama długość co `/Length`).
pub(super) const HIDDEN_LENGTH: &[u8] = b"AlfaLn";

/// Bajty po przygotowaniu.
pub(super) struct Prepared {
    /// Bajty dla `lopdf`.
    pub(super) bytes: Vec<u8>,
    /// Suma bezpośrednich `/Length` (nasycana).
    pub(super) direct_lengths: u64,
}

/// Początek pliku według `lopdf` (pierwszy `%PDF-`); pozycje strumieni liczone są od niego.
pub(super) fn pdf_start(bytes: &[u8]) -> usize {
    bytes.windows(5).position(|w| w == b"%PDF-").unwrap_or(0)
}

fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        0 | 9
            | 10
            | 12
            | 13
            | 32
            | b'('
            | b')'
            | b'<'
            | b'>'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'/'
            | b'%'
    )
}

fn is_space(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

/// Nazwa po rozwinięciu `#xx` (bez ukośnika).
fn decode_name(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let hex = raw
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (raw[i], hex) {
            (b'#', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    out
}

fn skip_space(bytes: &[u8], mut at: usize) -> usize {
    while bytes.get(at).copied().is_some_and(is_space) {
        at += 1;
    }
    at
}

/// Liczba całkowita bez znaku od `at`: (wartość nasycona, pozycja za nią).
fn integer(bytes: &[u8], at: usize) -> Option<(u64, usize)> {
    let mut end = at;
    let mut value = 0u64;
    while let Some(d) = bytes.get(end).filter(|b| b.is_ascii_digit()) {
        value = value.saturating_mul(10).saturating_add(u64::from(d - b'0'));
        end += 1;
    }
    (end > at && bytes.get(end).is_none_or(|&b| is_delimiter(b))).then_some((value, end))
}

/// Wartość po `/Length`: `Some(None)` — referencja `N G R`, `Some(Some(n))` — liczba.
fn length_value(bytes: &[u8], at: usize) -> Option<Option<u64>> {
    let (value, after) = integer(bytes, skip_space(bytes, at))?;
    let generation = skip_space(bytes, after);
    if let Some((_, after_gen)) = integer(bytes, generation).filter(|_| generation > after) {
        let r = skip_space(bytes, after_gen);
        if r > after_gen
            && bytes.get(r) == Some(&b'R')
            && bytes.get(r + 1).is_none_or(|&b| is_delimiter(b))
        {
            return Some(None);
        }
    }
    Some(Some(value))
}

/// Nadpisuje nazwę `[at, end)` nazwą `/hidden` dopełnioną spacjami.
fn hide(bytes: &mut [u8], at: usize, end: usize, hidden: &[u8]) {
    let replacement = [b"/".as_slice(), hidden].concat();
    for (i, slot) in bytes[at..end].iter_mut().enumerate() {
        *slot = replacement.get(i).copied().unwrap_or(b' ');
    }
}

/// Przygotowuje bajty (opis w nagłówku modułu).
pub(super) fn prepare(input: &[u8]) -> Prepared {
    let mut bytes = input.to_vec();
    let mut direct_lengths = 0u64;
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != b'/' {
            at += 1;
            continue;
        }
        let mut end = at + 1;
        while bytes.get(end).is_some_and(|&b| !is_delimiter(b)) {
            end += 1;
        }
        match decode_name(&bytes[at + 1..end]).as_slice() {
            b"Encrypt" => hide(&mut bytes, at, end, HIDDEN_ENCRYPT),
            b"Length" => match length_value(&bytes, end) {
                Some(None) => hide(&mut bytes, at, end, HIDDEN_LENGTH),
                Some(Some(n)) => direct_lengths = direct_lengths.saturating_add(n),
                None => {}
            },
            _ => {}
        }
        at = end;
    }
    Prepared {
        bytes,
        direct_lengths,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_keys_with_the_same_length_and_sums_direct_lengths() {
        let input = b"<< /Length 12 0 R /Len#67th 7 >> trailer << /Encr#79pt 5 0 R /Encrypt 5 0 R \
              /EncryptMetadata true /Length1 9 /Length 3>>";
        let out = prepare(input);
        assert_eq!(out.bytes.len(), input.len());
        let text = String::from_utf8(out.bytes).unwrap();
        assert!(
            text.starts_with("<< /AlfaLn 12 0 R /Len#67th 7 >>"),
            "{text}"
        );
        assert!(
            text.contains("/AlfaEnc   5 0 R /AlfaEnc 5 0 R /EncryptMetadata true"),
            "{text}"
        );
        assert_eq!(out.direct_lengths, 10);
        assert_eq!(pdf_start(b"xx%PDF-1.7"), 2);
    }
}
