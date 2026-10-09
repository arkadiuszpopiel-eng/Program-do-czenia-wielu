//! Dekodowanie treści strony według kodowania podanego przez serwer. Sam UTF-8 nie wystarcza:
//! strony w `iso-8859-2` / `windows-1250` (nadal spotykane po polsku) wracały ze znakami
//! zastępczymi zamiast „ą ę ł", a UTF-16 był brany za treść nietekstową (bajty zerowe).

use encoding_rs::{Encoding, UTF_8};
use tools_common_contract::text;

/// Kodowanie z parametru `charset` nagłówka `Content-Type` (`None` — brak albo nieznana nazwa).
fn declared(content_type: Option<&str>) -> Option<&'static Encoding> {
    let label = content_type?
        .split(';')
        .skip(1)
        .filter_map(|part| part.split_once('='))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("charset"))
        .map(|(_, value)| value.trim().trim_matches(['"', '\'']))?;
    Encoding::for_label(label.as_bytes())
}

/// Tekst strony albo `None`, gdy treść jest nietekstowa. Znacznik BOM ma pierwszeństwo przed
/// nagłówkiem (jak w przeglądarkach); bez wskazania kodowania — UTF-8 ze sprawdzeniem bajtów
/// zerowych z [`text::decode_text`].
pub(crate) fn decode_page(body: &[u8], content_type: Option<&str>) -> Option<String> {
    let encoding = Encoding::for_bom(body)
        .map(|(encoding, _)| encoding)
        .or_else(|| declared(content_type));
    match encoding {
        Some(encoding) if encoding != UTF_8 => {
            let (decoded, _, _) = encoding.decode(body);
            // Zdekodowany tekst z dużym udziałem znaków zerowych to nadal dane binarne.
            let sample: Vec<char> = decoded.chars().take(8192).collect();
            let zeros = sample.iter().filter(|c| **c == '\0').count();
            (zeros * 100 < sample.len().max(1)).then(|| decoded.into_owned())
        }
        _ => text::decode_text(body),
    }
}

#[cfg(test)]
mod tests {
    use super::decode_page;

    /// N-01 (docs/reviews/2026-10-08-tools-shell-clipboard-net-review.md).
    #[test]
    fn declared_charset_and_bom_are_honoured() {
        // „żółć" w windows-1250 i iso-8859-2 (te same bajty dla tych liter).
        let legacy = [0xBF, 0xF3, 0xB3, 0xE6];
        for content_type in [
            "text/html; charset=windows-1250",
            "text/html;charset=ISO-8859-2",
            "text/plain; boundary=x; charset=\"iso-8859-2\"",
        ] {
            assert_eq!(
                decode_page(&legacy, Some(content_type)).as_deref(),
                Some("żółć"),
                "{content_type}"
            );
        }
        // UTF-16 LE z BOM: wcześniej „treść nietekstowa" przez bajty zerowe.
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("zażółć".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(
            decode_page(&utf16, Some("text/plain")).as_deref(),
            Some("zażółć")
        );
    }

    #[test]
    fn utf8_and_binary_behave_as_before() {
        let utf8 = "zażółć gęślą jaźń".as_bytes();
        for content_type in [None, Some("text/html"), Some("text/html; charset=utf-8")] {
            assert_eq!(
                decode_page(utf8, content_type).as_deref(),
                Some("zażółć gęślą jaźń")
            );
        }
        // Nieznana nazwa kodowania — jak brak wskazania.
        assert_eq!(
            decode_page(utf8, Some("text/html; charset=nie-ma-takiego")).as_deref(),
            Some("zażółć gęślą jaźń")
        );
        let binary = [0u8, 1, 0, 2, 0, 3, 0, 0];
        assert_eq!(decode_page(&binary, Some("text/plain")), None);
        assert_eq!(
            decode_page(&binary, Some("text/plain; charset=windows-1250")),
            None
        );
    }
}
