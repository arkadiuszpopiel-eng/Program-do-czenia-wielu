//! Limity załączników i klasyfikacja pliku: rodzaj (obraz / tekst / dokument), co trafi do modelu
//! i szacunek tokenów wejścia (ta sama heurystyka co `providers_contract::estimate_input_tokens`).

use app_api::dto::{AttachmentDelivery, AttachmentKind};
use providers_contract::IMAGE_TOKEN_ESTIMATE;

/// Limity jednej wiadomości.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachmentLimits {
    /// Najwięcej plików w jednej wiadomości.
    pub max_files: usize,
    /// Największy pojedynczy plik (bajty).
    pub max_file_bytes: u64,
    /// Suma rozmiarów plików jednej wiadomości (bajty).
    pub max_total_bytes: u64,
    /// Największy obraz wysyłany modelowi w całości (większy — tylko metadane).
    pub max_image_bytes: u64,
    /// Najwięcej znaków tekstu jednego pliku dla modelu (reszta ucięta).
    pub max_text_chars: usize,
}

impl Default for AttachmentLimits {
    fn default() -> Self {
        Self {
            max_files: 10,
            max_file_bytes: 25 * MIB,
            max_total_bytes: 100 * MIB,
            max_image_bytes: 5 * MIB,
            max_text_chars: 100_000,
        }
    }
}

const MIB: u64 = 1024 * 1024;

/// Narzut opisu załącznika (nazwa, typ, rozmiar, znaczniki treści niezaufanej) w tokenach.
pub const LABEL_TOKENS: u64 = 40;

/// Typy obrazów przekazywanych modelowi (bez SVG — to tekst XML, może nieść skrypty).
const IMAGE_MIMES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];

/// Typy tekstowe spoza `text/*`.
const TEXT_MIMES: [&str; 5] = [
    "application/json",
    "application/toml",
    "application/xml",
    "application/yaml",
    "image/svg+xml",
];

/// Czy bajty zaczynają się sygnaturą deklarowanego typu obrazu (nazwa pliku może kłamać).
pub fn image_magic_matches(mime: &str, head: &[u8]) -> bool {
    match mime {
        "image/png" => head.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => head.starts_with(&[0xFF, 0xD8, 0xFF]),
        "image/gif" => head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a"),
        "image/webp" => head.len() >= 12 && &head[..4] == b"RIFF" && &head[8..12] == b"WEBP",
        _ => false,
    }
}

/// Klasyfikacja pliku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Classified {
    /// Rodzaj.
    pub kind: AttachmentKind,
    /// Co trafi do modelu.
    pub delivery: AttachmentDelivery,
    /// Szacunek tokenów wejścia.
    pub tokens: u64,
}

/// Klasyfikuje plik po typie MIME, początku treści (`head`, ≥ 8 KiB albo cały plik), rozmiarze
/// i — dla tekstu — liczbie znaków (`chars`, policzonej przez wywołującego).
pub fn classify(
    mime: &str,
    head: &[u8],
    bytes: u64,
    chars: Option<usize>,
    limits: &AttachmentLimits,
) -> Classified {
    let metadata = |kind| Classified {
        kind,
        delivery: AttachmentDelivery::MetadataOnly,
        tokens: LABEL_TOKENS,
    };
    if IMAGE_MIMES.contains(&mime) {
        if !image_magic_matches(mime, head) {
            return metadata(AttachmentKind::Document);
        }
        if bytes > limits.max_image_bytes {
            return metadata(AttachmentKind::Image);
        }
        return Classified {
            kind: AttachmentKind::Image,
            delivery: AttachmentDelivery::Full,
            tokens: IMAGE_TOKEN_ESTIMATE + LABEL_TOKENS,
        };
    }
    let textual = mime.starts_with("text/")
        || TEXT_MIMES.contains(&mime)
        || (mime == "application/octet-stream" && !artifacts_contract::looks_binary(head));
    match chars {
        Some(chars) if textual && !artifacts_contract::looks_binary(head) => {
            let sent = chars.min(limits.max_text_chars);
            Classified {
                kind: AttachmentKind::Text,
                delivery: if chars > limits.max_text_chars {
                    AttachmentDelivery::Truncated
                } else {
                    AttachmentDelivery::Full
                },
                tokens: (sent as u64).div_ceil(4) + LABEL_TOKENS,
            }
        }
        _ => metadata(AttachmentKind::Document),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    #[test]
    fn images_need_matching_magic_and_size_limit() {
        let l = AttachmentLimits::default();
        let ok = classify("image/png", PNG, 1000, None, &l);
        assert_eq!(ok.kind, AttachmentKind::Image);
        assert_eq!(ok.delivery, AttachmentDelivery::Full);
        assert_eq!(ok.tokens, IMAGE_TOKEN_ESTIMATE + LABEL_TOKENS);
        let fake = classify("image/png", b"<script>", 10, None, &l);
        assert_eq!(fake.kind, AttachmentKind::Document);
        assert_eq!(fake.delivery, AttachmentDelivery::MetadataOnly);
        let big = classify("image/png", PNG, l.max_image_bytes + 1, None, &l);
        assert_eq!(
            (big.kind, big.delivery),
            (AttachmentKind::Image, AttachmentDelivery::MetadataOnly)
        );
        assert!(image_magic_matches("image/jpeg", &[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(image_magic_matches("image/gif", b"GIF89a.."));
        assert!(image_magic_matches("image/webp", b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(!image_magic_matches("image/webp", b"RIFF"));
    }

    #[test]
    fn text_is_estimated_and_truncated_at_limit() {
        let l = AttachmentLimits {
            max_text_chars: 100,
            ..AttachmentLimits::default()
        };
        let small = classify("text/plain", b"abc", 3, Some(3), &l);
        assert_eq!(small.kind, AttachmentKind::Text);
        assert_eq!(small.delivery, AttachmentDelivery::Full);
        assert_eq!(small.tokens, 1 + LABEL_TOKENS);
        let long = classify("text/markdown", b"#", 1000, Some(1000), &l);
        assert_eq!(long.delivery, AttachmentDelivery::Truncated);
        assert_eq!(long.tokens, 25 + LABEL_TOKENS);
        let unknown_text = classify("application/octet-stream", b"zwykly", 6, Some(6), &l);
        assert_eq!(unknown_text.kind, AttachmentKind::Text);
        let binary = classify("application/octet-stream", b"\0\x01\x02", 3, None, &l);
        assert_eq!(binary.kind, AttachmentKind::Document);
        let pdf = classify("application/pdf", b"%PDF-1.7", 8, None, &l);
        assert_eq!(pdf.delivery, AttachmentDelivery::MetadataOnly);
        let nul_in_text = classify("text/plain", b"a\0b", 3, Some(3), &l);
        assert_eq!(nul_in_text.kind, AttachmentKind::Document);
    }
}
