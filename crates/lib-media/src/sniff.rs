//! Rozpoznanie formatu po sygnaturze (pierwsze bajty) — nigdy po rozszerzeniu nazwy.

use crate::audio::{adts_header, mpeg_header};

/// Format kontenera rozpoznany po sygnaturze.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    /// PNG (także APNG).
    Png,
    /// JPEG/JFIF/EXIF.
    Jpeg,
    /// GIF 87a/89a.
    Gif,
    /// BMP (DIB).
    Bmp,
    /// WebP (RIFF).
    Webp,
    /// WAV (RIFF).
    Wav,
    /// MPEG audio (MP3, także z ID3v2).
    Mp3,
    /// AAC w ramkach ADTS.
    Aac,
    /// FLAC.
    Flac,
    /// Ogg (Vorbis, Opus, FLAC, Theora, Speex).
    Ogg,
    /// ISO BMFF: MP4, MOV, M4A, 3GP, HEIF/AVIF.
    Mp4,
    /// Matroska / WebM (EBML).
    Matroska,
    /// AVI (RIFF).
    Avi,
}

const BMP_DIB_SIZES: [u32; 6] = [12, 40, 52, 56, 108, 124];

/// Format po pierwszych bajtach pliku (`None` — nierozpoznany).
pub fn sniff(h: &[u8]) -> Option<Format> {
    let at = |i: usize, sig: &[u8]| h.get(i..i + sig.len()) == Some(sig);
    if at(0, b"\x89PNG\r\n\x1a\n") {
        return Some(Format::Png);
    }
    if at(0, &[0xFF, 0xD8, 0xFF]) {
        return Some(Format::Jpeg);
    }
    if at(0, b"GIF87a") || at(0, b"GIF89a") {
        return Some(Format::Gif);
    }
    if at(0, b"RIFF") {
        return match h.get(8..12) {
            Some(b"WAVE") => Some(Format::Wav),
            Some(b"WEBP") => Some(Format::Webp),
            Some(b"AVI ") => Some(Format::Avi),
            _ => None,
        };
    }
    if at(0, b"fLaC") {
        return Some(Format::Flac);
    }
    if at(0, b"OggS") {
        return Some(Format::Ogg);
    }
    if at(4, b"ftyp") {
        return Some(Format::Mp4);
    }
    if at(0, &[0x1A, 0x45, 0xDF, 0xA3]) {
        return Some(Format::Matroska);
    }
    if at(0, b"ID3") {
        return Some(Format::Mp3);
    }
    if at(0, b"BM")
        && let Some(dib) = h.get(14..18)
        && let Ok(bytes) = <[u8; 4]>::try_from(dib)
        && BMP_DIB_SIZES.contains(&u32::from_le_bytes(bytes))
    {
        return Some(Format::Bmp);
    }
    let first = h.get(..4)?;
    if adts_header(first).is_some() {
        return Some(Format::Aac);
    }
    if mpeg_header(first).is_some() {
        return Some(Format::Mp3);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures() {
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\nrest"), Some(Format::Png));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(Format::Jpeg));
        assert_eq!(sniff(b"GIF89a.."), Some(Format::Gif));
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt "), Some(Format::Wav));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(Format::Webp));
        assert_eq!(sniff(b"RIFF\0\0\0\0AVI LIST"), Some(Format::Avi));
        assert_eq!(sniff(b"RIFF\0\0\0\0XXXX"), None);
        assert_eq!(sniff(b"fLaC\0"), Some(Format::Flac));
        assert_eq!(sniff(b"OggS\0"), Some(Format::Ogg));
        assert_eq!(sniff(b"\0\0\0\x18ftypisom"), Some(Format::Mp4));
        assert_eq!(sniff(&[0x1A, 0x45, 0xDF, 0xA3, 0]), Some(Format::Matroska));
        assert_eq!(sniff(b"ID3\x04\0"), Some(Format::Mp3));
        let mut bmp = b"BM".to_vec();
        bmp.resize(14, 0);
        bmp.extend_from_slice(&40u32.to_le_bytes());
        assert_eq!(sniff(&bmp), Some(Format::Bmp));
        assert_eq!(sniff(b"BM tekst bez naglowka DIB"), None);
        assert_eq!(sniff(&[0xFF, 0xFB, 0x90, 0x64]), Some(Format::Mp3));
        assert_eq!(sniff(&[0xFF, 0xF1, 0x50, 0x80]), Some(Format::Aac));
        assert_eq!(sniff(b"zwykly tekst"), None);
        assert_eq!(sniff(b""), None);
    }
}
