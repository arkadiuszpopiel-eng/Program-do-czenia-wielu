//! Kodowanie czcionek prostych: tabela 256 kodów z kodowania bazowego (Standard, WinAnsi,
//! MacRoman, MacExpert — tabele `lopdf`) i własnych `Differences`. Nazwy glifów: lista Adobe
//! (przez `lopdf`, nazwa po nazwie — nieznana nie psuje całej tablicy), `uniXXXX`, `uXXXX`,
//! przyrostki `.sc` i ligatury `f_f`.

use lopdf::{Dictionary, Document, Object};

use super::load::{get, get_name, resolve};

/// Najwięcej elementów tablicy `Differences`.
const MAX_DIFFERENCES: usize = 1024;

/// Kodowania bazowe znane `lopdf`.
const BASES: [&[u8]; 4] = [
    b"StandardEncoding",
    b"WinAnsiEncoding",
    b"MacRomanEncoding",
    b"MacExpertEncoding",
];

/// Słownik czcionki Type1 z podanym kodowaniem (do odpytania tabel `lopdf`).
fn synthetic(encoding: Object) -> Dictionary {
    let mut font = Dictionary::new();
    font.set("Type", Object::Name(b"Font".to_vec()));
    font.set("Subtype", Object::Name(b"Type1".to_vec()));
    font.set("Encoding", encoding);
    font
}

/// Tekst kodów `codes` czcionki (puste, gdy kodowania nie da się odczytać).
fn decode_codes(
    doc: &Document,
    font: &Dictionary,
    codes: std::ops::RangeInclusive<u8>,
) -> Vec<String> {
    match font.get_font_encoding(doc) {
        Ok(encoding) => codes
            .map(|b| encoding.bytes_to_string(&[b]).unwrap_or_default())
            .collect(),
        Err(_) => codes.map(|_| String::new()).collect(),
    }
}

/// Cyfry szesnastkowe jako tekst (`None`, gdy jest inny znak).
fn hex(h: &[u8]) -> Option<&str> {
    std::str::from_utf8(h)
        .ok()
        .filter(|s| s.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Część nazwy glifu (bez `_`): `uniXXXX…`, `uXXXX[XX]` albo nazwa z listy Adobe.
fn glyph_part(doc: &Document, part: &[u8]) -> String {
    if let Some(digits) = part.strip_prefix(b"uni").and_then(hex)
        && !digits.is_empty()
        && digits.len() % 4 == 0
    {
        let units: Vec<u16> = (0..digits.len())
            .step_by(4)
            .filter_map(|i| digits.get(i..i + 4))
            .filter_map(|d| u16::from_str_radix(d, 16).ok())
            .collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(digits) = part.strip_prefix(b"u").and_then(hex)
        && (4..=6).contains(&digits.len())
    {
        return u32::from_str_radix(digits, 16)
            .ok()
            .and_then(char::from_u32)
            .map(String::from)
            .unwrap_or_default();
    }
    let mut encoding = Dictionary::new();
    encoding.set("Type", Object::Name(b"Encoding".to_vec()));
    encoding.set(
        "Differences",
        Object::Array(vec![Object::Integer(0), Object::Name(part.to_vec())]),
    );
    // Kod 0 nie ma znaku w StandardEncoding: nieznana nazwa da pusty tekst, nie „A”.
    decode_codes(doc, &synthetic(Object::Dictionary(encoding)), 0..=0)
        .pop()
        .unwrap_or_default()
}

/// Tekst nazwy glifu (przyrostek po kropce pomijany, `_` łączy składniki ligatury).
pub(super) fn glyph_text(doc: &Document, name: &[u8]) -> String {
    let base = name.split(|&b| b == b'.').next().unwrap_or_default();
    if base.is_empty() {
        return String::new();
    }
    base.split(|&b| b == b'_')
        .map(|part| glyph_part(doc, part))
        .collect()
}

/// Tabela 256 kodów czcionki prostej (kodowanie bazowe + `Differences`).
pub(super) fn simple_table(doc: &Document, font: &Dictionary) -> Vec<String> {
    let (base, differences) = match get(doc, font, b"Encoding") {
        Some(Object::Name(name)) => (name.as_slice(), None),
        Some(Object::Dictionary(enc)) => (
            get_name(doc, enc, b"BaseEncoding").unwrap_or(b"StandardEncoding"),
            get(doc, enc, b"Differences").and_then(|o| o.as_array().ok()),
        ),
        _ => (b"StandardEncoding".as_slice(), None),
    };
    let base = BASES
        .into_iter()
        .find(|b| *b == base)
        .unwrap_or(b"StandardEncoding");
    let mut table = decode_codes(doc, &synthetic(Object::Name(base.to_vec())), 0..=255);
    let mut code: Option<usize> = None;
    for item in differences.into_iter().flatten().take(MAX_DIFFERENCES) {
        match resolve(doc, item).map(|(_, o)| o) {
            Some(Object::Integer(i)) => code = usize::try_from(*i).ok().filter(|c| *c < 256),
            Some(Object::Name(name)) => {
                if let Some(c) = code
                    && let Some(slot) = table.get_mut(c)
                {
                    *slot = glyph_text(doc, name);
                    code = Some(c + 1).filter(|c| *c < 256);
                }
            }
            _ => {}
        }
    }
    table
}
