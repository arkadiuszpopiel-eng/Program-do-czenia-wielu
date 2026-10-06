//! Obrazy: PNG (IHDR, APNG `acTL`), JPEG (pierwszy SOF), GIF (ekran logiczny, klatki
//! i opóźnienia), BMP (nagłówek DIB), WebP (VP8, VP8L, VP8X).

use crate::{MediaError, MediaInfo, MediaKind, Reader, be16, be32, byte, le16, le32, malformed};

fn image(r: &Reader<'_>, format: &str, mime: &str) -> MediaInfo {
    let mut info = MediaInfo::new(MediaKind::Image, format, mime, r.size());
    info.codecs = vec![format.to_owned()];
    info
}

fn dims(info: &mut MediaInfo, w: u32, h: u32) -> Result<(), MediaError> {
    if w == 0 || h == 0 {
        return Err(malformed("zerowy wymiar obrazu"));
    }
    info.width = Some(w);
    info.height = Some(h);
    Ok(())
}

/// PNG: IHDR (wymiary, głębia, typ koloru) i `acTL` przed pierwszym IDAT (APNG).
pub(crate) fn png(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let h = r.exact(8, 25, "PNG IHDR")?;
    if h.get(4..8) != Some(b"IHDR") {
        return Err(malformed("PNG bez IHDR"));
    }
    let mut info = image(r, "png", "image/png");
    dims(&mut info, be32(&h, 8)?, be32(&h, 12)?)?;
    info.bits_per_sample = Some(u16::from(byte(&h, 16)?));
    info.channels = match byte(&h, 17)? {
        0 | 3 => Some(1),
        4 => Some(2),
        2 => Some(3),
        6 => Some(4),
        _ => return Err(malformed("PNG: nieznany typ koloru")),
    };
    let mut pos = 8u64;
    while pos < r.size() {
        r.step()?;
        let chunk = r.get(pos, 8)?;
        if chunk.len() < 8 {
            info.partial = true;
            break;
        }
        let len = u64::from(be32(&chunk, 0)?);
        match chunk.get(4..8) {
            Some(b"acTL") => {
                let d = r.exact(pos.saturating_add(8), 8, "PNG acTL")?;
                info.frames = Some(be32(&d, 0)?);
            }
            Some(b"IDAT") | Some(b"IEND") => break,
            _ => {}
        }
        pos = pos.saturating_add(12).saturating_add(len);
    }
    Ok(info)
}

fn is_sof(marker: u8) -> bool {
    (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC)
}

/// JPEG: segmenty do pierwszego SOF (wymiary, precyzja, składowe).
pub(crate) fn jpeg(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let mut pos = 2u64;
    loop {
        r.step()?;
        let m = r.get(pos, 2)?;
        if m.len() < 2 {
            return Err(MediaError::Truncated("JPEG bez SOF"));
        }
        if m[0] != 0xFF {
            return Err(malformed("JPEG: oczekiwano znacznika segmentu"));
        }
        let marker = m[1];
        match marker {
            0xFF => {
                pos += 1;
                continue;
            }
            0x01 | 0xD0..=0xD8 => {
                pos += 2;
                continue;
            }
            0xD9 | 0xDA => return Err(MediaError::Truncated("JPEG: dane obrazu przed SOF")),
            _ => {}
        }
        let len = u64::from(be16(
            &r.exact(pos.saturating_add(2), 2, "JPEG segment")?,
            0,
        )?);
        if len < 2 {
            return Err(malformed("JPEG: długość segmentu < 2"));
        }
        if is_sof(marker) {
            let d = r.exact(pos.saturating_add(4), 6, "JPEG SOF")?;
            let mut info = image(r, "jpeg", "image/jpeg");
            dims(&mut info, u32::from(be16(&d, 3)?), u32::from(be16(&d, 1)?))?;
            info.bits_per_sample = Some(u16::from(byte(&d, 0)?));
            info.channels = Some(u16::from(byte(&d, 5)?));
            if marker == 0xC2 {
                info.codecs = vec!["jpeg (progresywny)".into()];
            }
            return Ok(info);
        }
        pos = pos.saturating_add(2).saturating_add(len);
    }
}

fn color_table(flags: u8) -> u64 {
    if flags & 0x80 == 0 {
        0
    } else {
        3 * (1u64 << ((flags & 0x07) + 1))
    }
}

fn skip_sub_blocks(r: &mut Reader<'_>, mut pos: u64) -> Result<Option<u64>, MediaError> {
    loop {
        r.step()?;
        let b = r.get(pos, 1)?;
        let Some(&n) = b.first() else {
            return Ok(None);
        };
        pos = pos.saturating_add(1 + u64::from(n));
        if n == 0 {
            return Ok(Some(pos));
        }
    }
}

/// GIF: wymiary ekranu logicznego, liczba klatek i czas animacji (suma opóźnień GCE).
/// Po limicie odczytu wynik jest częściowy (bez liczby klatek), a nie błędem.
pub(crate) fn gif(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let h = r.exact(0, 13, "GIF")?;
    let mut info = image(r, "gif", "image/gif");
    dims(&mut info, u32::from(le16(&h, 6)?), u32::from(le16(&h, 8)?))?;
    info.channels = Some(3);
    match gif_frames(r, 13 + color_table(byte(&h, 10)?)) {
        Ok((frames, delay_cs, complete)) => {
            info.frames = Some(frames);
            info.partial = !complete;
            if frames > 1 {
                info.duration_ms = Some(delay_cs.saturating_mul(10));
            }
        }
        Err(MediaError::Limit(_)) => info.partial = true,
        Err(e) => return Err(e),
    }
    Ok(info)
}

fn gif_frames(r: &mut Reader<'_>, mut pos: u64) -> Result<(u32, u64, bool), MediaError> {
    let (mut frames, mut delay) = (0u32, 0u64);
    loop {
        r.step()?;
        let Some(&kind) = r.get(pos, 1)?.first() else {
            return Ok((frames, delay, false));
        };
        let next = match kind {
            0x3B => return Ok((frames, delay, true)),
            0x2C => {
                frames = frames.saturating_add(1);
                let d = r.exact(pos, 10, "GIF obraz")?;
                let start = pos
                    .saturating_add(11)
                    .saturating_add(color_table(byte(&d, 9)?));
                skip_sub_blocks(r, start)?
            }
            0x21 => {
                let label = byte(&r.exact(pos, 2, "GIF rozszerzenie")?, 1)?;
                if label == 0xF9 {
                    let g = r.exact(pos.saturating_add(2), 6, "GIF GCE")?;
                    delay = delay.saturating_add(u64::from(le16(&g, 2)?));
                }
                skip_sub_blocks(r, pos.saturating_add(2))?
            }
            _ => return Ok((frames, delay, false)),
        };
        match next {
            Some(p) => pos = p,
            None => return Ok((frames, delay, false)),
        }
    }
}

/// BMP: nagłówek DIB (OS/2 `BITMAPCOREHEADER` albo `BITMAPINFOHEADER` i nowsze).
pub(crate) fn bmp(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let h = r.exact(0, 30, "BMP")?;
    let mut info = image(r, "bmp", "image/bmp");
    let (w, hgt, bpp) = if le32(&h, 14)? == 12 {
        (
            u32::from(le16(&h, 18)?),
            u32::from(le16(&h, 20)?),
            le16(&h, 24)?,
        )
    } else {
        let w = le32(&h, 18)? as i32;
        let hgt = le32(&h, 22)? as i32;
        (w.unsigned_abs(), hgt.unsigned_abs(), le16(&h, 28)?)
    };
    dims(&mut info, w, hgt)?;
    info.bits_per_sample = Some(bpp);
    Ok(info)
}

fn le24(b: &[u8], i: usize) -> Result<u32, MediaError> {
    Ok(u32::from(byte(b, i)?) | u32::from(byte(b, i + 1)?) << 8 | u32::from(byte(b, i + 2)?) << 16)
}

/// WebP: VP8 (stratny), VP8L (bezstratny), VP8X (rozszerzony, animacja).
pub(crate) fn webp(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let h = r.exact(0, 30, "WebP")?;
    let mut info = image(r, "webp", "image/webp");
    match h.get(12..16) {
        Some(b"VP8 ") => {
            if h.get(23..26) != Some(&[0x9D, 0x01, 0x2A]) {
                return Err(malformed("WebP VP8: brak kodu startu"));
            }
            info.codecs = vec!["vp8".into()];
            dims(
                &mut info,
                u32::from(le16(&h, 26)? & 0x3FFF),
                u32::from(le16(&h, 28)? & 0x3FFF),
            )?;
        }
        Some(b"VP8L") => {
            if byte(&h, 20)? != 0x2F {
                return Err(malformed("WebP VP8L: brak sygnatury"));
            }
            let bits = le32(&h, 21)?;
            info.codecs = vec!["vp8l".into()];
            dims(&mut info, (bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1)?;
        }
        Some(b"VP8X") => {
            let flags = byte(&h, 20)?;
            dims(&mut info, le24(&h, 24)? + 1, le24(&h, 27)? + 1)?;
            if flags & 0x02 != 0 {
                info.codecs = vec!["webp (animowany)".into()];
            }
        }
        _ => return Err(malformed("WebP: nieznany blok obrazu")),
    }
    Ok(info)
}
