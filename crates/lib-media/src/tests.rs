//! Parsery na minimalnych plikach z `samples` i na plikach złośliwych (rozmiary, pętle, zera).

use crate::samples;
use crate::*;

fn info_of(bytes: &[u8]) -> MediaInfo {
    probe_bytes(bytes).unwrap()
}

#[test]
fn images() {
    let png = info_of(&samples::png(640, 480, 0));
    assert_eq!((png.kind, png.format.as_str()), (MediaKind::Image, "png"));
    assert_eq!((png.width, png.height), (Some(640), Some(480)));
    assert_eq!(
        (png.channels, png.bits_per_sample, png.frames),
        (Some(4), Some(8), None)
    );
    assert_eq!(png.pixels(), Some(640 * 480));
    assert_eq!(info_of(&samples::png(10, 10, 12)).frames, Some(12));
    let jpg = info_of(&samples::jpeg(1920, 1080));
    assert_eq!(
        (jpg.width, jpg.height, jpg.channels),
        (Some(1920), Some(1080), Some(3))
    );
    assert_eq!(jpg.mime, "image/jpeg");
    let gif = info_of(&samples::gif(32, 16, 3, 5));
    assert_eq!(
        (gif.width, gif.frames, gif.duration_ms),
        (Some(32), Some(3), Some(150))
    );
    assert!(!gif.partial);
    let still = info_of(&samples::gif(8, 8, 1, 0));
    assert_eq!((still.frames, still.duration_ms), (Some(1), None));
    let bmp = info_of(&samples::bmp(300, -200));
    assert_eq!(
        (bmp.width, bmp.height, bmp.bits_per_sample),
        (Some(300), Some(200), Some(24))
    );
    let webp = info_of(&samples::webp_lossless(4000, 3000));
    assert_eq!((webp.width, webp.height), (Some(4000), Some(3000)));
    assert_eq!(webp.codecs, vec!["vp8l"]);
}

#[test]
fn audio() {
    let wav = info_of(&samples::wav(16_000, 1, 16, 1500));
    assert_eq!((wav.kind, wav.format.as_str()), (MediaKind::Audio, "wav"));
    assert_eq!(
        (wav.sample_rate, wav.channels, wav.bits_per_sample),
        (Some(16_000), Some(1), Some(16))
    );
    assert_eq!(wav.duration_ms, Some(1500));
    assert_eq!(wav.codecs, vec!["pcm_s16le"]);
    assert_eq!(wav.bit_rate, Some(256_000));
    let mp3 = info_of(&samples::mp3(1000));
    assert_eq!(
        (mp3.sample_rate, mp3.channels, mp3.frames),
        (Some(44_100), Some(2), Some(1000))
    );
    assert_eq!(mp3.duration_ms, Some(1000 * 1152 * 1000 / 44_100));
    let flac = info_of(&samples::flac(48_000, 2, 96_000));
    assert_eq!(
        (flac.sample_rate, flac.channels, flac.bits_per_sample),
        (Some(48_000), Some(2), Some(16))
    );
    assert_eq!(flac.duration_ms, Some(2000));
    let opus = info_of(&samples::ogg_opus(2500));
    assert_eq!(
        (opus.codecs.clone(), opus.channels),
        (vec!["opus".to_owned()], Some(2))
    );
    assert_eq!(opus.duration_ms, Some(2500));
    let adts = info_of(&[0xFF, 0xF1, 0x50, 0x80, 0, 0, 0]);
    assert_eq!(
        (adts.format.as_str(), adts.sample_rate, adts.channels),
        ("aac", Some(44_100), Some(2))
    );
}

#[test]
fn mp4_tree_with_moov_before_and_after_mdat() {
    for last in [false, true] {
        let mp4 = info_of(&samples::mp4(1280, 720, 90_500, last));
        assert_eq!((mp4.kind, mp4.format.as_str()), (MediaKind::Video, "mp4"));
        assert_eq!((mp4.width, mp4.height), (Some(1280), Some(720)));
        assert_eq!(mp4.duration_ms, Some(90_500));
        assert_eq!(mp4.codecs, vec!["h264", "aac"]);
        assert_eq!((mp4.sample_rate, mp4.channels), (Some(44_100), Some(2)));
    }
}

#[test]
fn unknown_and_truncated() {
    assert_eq!(probe_bytes(b"to jest tekst"), Err(MediaError::Unknown));
    assert_eq!(probe_bytes(b""), Err(MediaError::Unknown));
    assert!(probe_bytes(&samples::png(1, 1, 0)[..20]).is_err());
    assert!(probe_bytes(&samples::jpeg(1, 1)[..12]).is_err());
    assert!(probe_bytes(&samples::flac(1, 1, 1)[..30]).is_err());
}

#[test]
fn malicious_headers_fail_cleanly() {
    // PNG o zerowej szerokości.
    assert!(matches!(
        probe_bytes(&samples::png(0, 5, 0)),
        Err(MediaError::Malformed(_))
    ));
    // WAV z byte_rate = 0 (dzielenie przez zero) — czas nieznany, bez paniki.
    let mut wav = samples::wav(8000, 1, 16, 100);
    wav[28..32].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(info_of(&wav).duration_ms, None);
    // FLAC z częstotliwością 0.
    assert_eq!(info_of(&samples::flac(0, 1, 1000)).duration_ms, None);
    // MP4: pudełko o rozmiarze 4 (< nagłówek), rozmiar 64-bit poza plikiem.
    let mut tiny = b"\0\0\0\x10ftypisom\0\0\0\0".to_vec();
    tiny.extend_from_slice(&[0, 0, 0, 4, b'm', b'o', b'o', b'v']);
    assert!(matches!(probe_bytes(&tiny), Err(MediaError::Malformed(_))));
    let mut big = b"\0\0\0\x10ftypisom\0\0\0\0".to_vec();
    big.extend_from_slice(&[0, 0, 0, 1, b'm', b'o', b'o', b'v']);
    big.extend_from_slice(&u64::MAX.to_be_bytes());
    assert!(matches!(probe_bytes(&big), Err(MediaError::Malformed(_))));
}

#[test]
fn nesting_and_step_limits() {
    // 64 zagnieżdżone `trak` — głębokość ponad limit.
    let mut inner = Vec::new();
    for _ in 0..64 {
        let mut b = (8 + inner.len() as u32).to_be_bytes().to_vec();
        b.extend_from_slice(b"trak");
        b.extend_from_slice(&inner);
        inner = b;
    }
    let mut moov = (8 + inner.len() as u32).to_be_bytes().to_vec();
    moov.extend_from_slice(b"moov");
    moov.extend_from_slice(&inner);
    let mut file = b"\0\0\0\x10ftypisom\0\0\0\0".to_vec();
    file.extend_from_slice(&moov);
    assert!(matches!(probe_bytes(&file), Err(MediaError::Limit(_))));
    // Tysiące pustych pudełek — limit kroków.
    let mut many = b"\0\0\0\x10ftypisom\0\0\0\0".to_vec();
    for _ in 0..2000 {
        many.extend_from_slice(&[0, 0, 0, 8, b'f', b'r', b'e', b'e']);
    }
    let limits = Limits {
        max_steps: 500,
        ..Limits::default()
    };
    assert!(matches!(
        crate::probe(&mut SliceSource::new(&many), &limits),
        Err(MediaError::Limit(_))
    ));
    // JPEG z samymi bajtami wypełnienia.
    let mut fill = vec![0xFF, 0xD8];
    fill.resize(300_000, 0xFF);
    assert!(probe_bytes(&fill).is_err());
}

#[test]
fn read_budget_is_enforced_for_large_sources() {
    struct Huge;
    impl ByteSource for Huge {
        fn size(&self) -> u64 {
            1 << 40
        }
        fn read_at(&mut self, offset: u64, len: usize) -> Result<Vec<u8>, MediaError> {
            // Plik złożony z pustych pudełek `free` po nagłówku `ftyp`.
            let mut out = Vec::with_capacity(len);
            for i in 0..len as u64 {
                let at = offset + i;
                let b = if at < 16 {
                    b"\0\0\0\x10ftypisom\0\0\0\0"[at as usize]
                } else {
                    [0, 0, 0x10, 0, b'f', b'r', b'e', b'e'][((at - 16) % 8) as usize]
                };
                out.push(b);
            }
            Ok(out)
        }
    }
    let limits = Limits {
        max_read_bytes: 1 << 20,
        ..Limits::default()
    };
    let err = crate::probe(&mut Huge, &limits).unwrap_err();
    assert!(
        matches!(err, MediaError::Limit(_) | MediaError::Malformed(_)),
        "{err:?}"
    );
}

#[test]
fn info_serializes_for_tools() {
    let v = serde_json::to_value(info_of(&samples::wav(8000, 2, 16, 10))).unwrap();
    assert_eq!(v["kind"], "audio");
    assert_eq!(v["sample_rate"], 8000);
}

#[test]
fn containers_brands_and_box_sizes() {
    let mut mov = samples::mp4(640, 360, 1000, false);
    mov[8..12].copy_from_slice(b"qt  ");
    let mov = info_of(&mov);
    assert_eq!(
        (mov.format.as_str(), mov.mime.as_str()),
        ("mov", "video/quicktime")
    );
    let mut heic = samples::mp4(64, 64, 0, false);
    heic[8..12].copy_from_slice(b"heic");
    let heic = info_of(&heic);
    assert_eq!(
        (heic.kind, heic.format.as_str(), heic.partial),
        (MediaKind::Image, "heif", true)
    );
    let m4a = info_of(&samples::m4a(2500));
    assert_eq!(
        (m4a.kind, m4a.format.as_str(), m4a.duration_ms),
        (MediaKind::Audio, "m4a", Some(2500))
    );
    assert_eq!(m4a.codecs, vec!["aac"]);
    // `isom` bez ścieżki wideo → dźwięk M4A.
    let mut iso_audio = samples::m4a(100);
    iso_audio[8..12].copy_from_slice(b"isom");
    assert_eq!(info_of(&iso_audio).mime, "audio/mp4");
    // Pudełko z rozmiarem 64-bit i ostatnie pudełko „do końca pliku” (rozmiar 0).
    let mut big = samples::mp4(32, 32, 10, false);
    let mut free = vec![0, 0, 0, 1, b'f', b'r', b'e', b'e'];
    free.extend_from_slice(&24u64.to_be_bytes());
    free.extend_from_slice(&[0; 8]);
    big.extend_from_slice(&free);
    big.extend_from_slice(&[0, 0, 0, 0, b'm', b'd', b'a', b't', 1, 2, 3]);
    assert_eq!(info_of(&big).width, Some(32));
    let webm = info_of(&[
        0x1A, 0x45, 0xDF, 0xA3, 0x42, 0x82, 0x84, b'w', b'e', b'b', b'm',
    ]);
    assert_eq!((webm.format.as_str(), webm.partial), ("webm", true));
    assert_eq!(info_of(&[0x1A, 0x45, 0xDF, 0xA3, 0, 0]).format, "matroska");
    let avi = info_of(&samples::avi(320, 240, 250, 40_000));
    assert_eq!(
        (avi.width, avi.frames, avi.duration_ms),
        (Some(320), Some(250), Some(10_000))
    );
    assert!(probe_bytes(b"RIFF\0\0\0\0AVI LIST").is_err());
}

#[test]
fn codec_names_are_sanitized() {
    let mut odd = samples::mp4(16, 16, 10, false);
    let at = odd.windows(4).rposition(|w| w == b"avc1").unwrap();
    odd[at..at + 4].copy_from_slice(b"x<1>");
    let info = info_of(&odd);
    assert_eq!(info.codecs[0], "x1");
}
