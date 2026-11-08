//! Obrazy schowka bez zależności: DIB (`CF_DIB`/`CF_DIBV5`, 24/32 bpp) → RGBA → PNG
//! (deflate „stored”, bez kompresji — poprawny PNG, rozmiar ≈ surowe piksele).

use platform_contract::PlatformError;

/// Maksymalna liczba pikseli obrazu ze schowka (ochrona pamięci).
const MAX_PIXELS: u64 = 64 * 1024 * 1024;

/// Obraz RGBA 8 bit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rgba {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn i32_at(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn unsupported(what: &str) -> PlatformError {
    PlatformError::Unsupported(format!("obraz w schowku: {what}"))
}

/// Dekoduje DIB (nagłówek `BITMAPINFOHEADER`/`V4`/`V5` + piksele) do RGBA.
pub(crate) fn dib_to_rgba(dib: &[u8]) -> Result<Rgba, PlatformError> {
    let bad = || unsupported("uszkodzony nagłówek DIB");
    let header = u32_at(dib, 0).ok_or_else(bad)?;
    let width = i32_at(dib, 4).ok_or_else(bad)?;
    let height = i32_at(dib, 8).ok_or_else(bad)?;
    let bpp = u16_at(dib, 14).ok_or_else(bad)?;
    let compression = u32_at(dib, 16).ok_or_else(bad)?;
    if header < 40 || width <= 0 || height == 0 {
        return Err(bad());
    }
    // BI_RGB = 0, BI_BITFIELDS = 3 (maski po nagłówku 40 B; w V4/V5 są w nagłówku).
    let masks_after_header = if compression == 3 && header == 40 {
        12
    } else {
        0
    };
    if !(compression == 0 || compression == 3) || !(bpp == 24 || bpp == 32) {
        return Err(unsupported(&format!("{bpp} bpp, kompresja {compression}")));
    }
    let w = width.unsigned_abs();
    let h = height.unsigned_abs();
    if u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(unsupported("za duży"));
    }
    let stride = (w as usize * usize::from(bpp)).div_ceil(32) * 4;
    let offset = header as usize + masks_after_header;
    let needed = offset + stride * h as usize;
    if dib.len() < needed {
        return Err(bad());
    }
    let bytes_pp = usize::from(bpp / 8);
    let mut pixels = Vec::with_capacity(w as usize * h as usize * 4);
    for row in 0..h as usize {
        // Wysokość dodatnia = obraz od dołu.
        let src_row = if height > 0 {
            h as usize - 1 - row
        } else {
            row
        };
        let start = offset + src_row * stride;
        for x in 0..w as usize {
            let p = &dib[start + x * bytes_pp..start + (x + 1) * bytes_pp];
            let alpha = if bytes_pp == 4 { p[3] } else { 255 };
            pixels.extend_from_slice(&[p[2], p[1], p[0], alpha]);
        }
    }
    // 32 bpp BI_RGB zwykle ma kanał alfa = 0 (nieużywany) → obraz nieprzezroczysty.
    if bytes_pp == 4 && pixels.chunks_exact(4).all(|px| px[3] == 0) {
        pixels.chunks_exact_mut(4).for_each(|px| px[3] = 255);
    }
    Ok(Rgba {
        width: w,
        height: h,
        pixels,
    })
}

const fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
}

static CRC_TABLE: [u32; 256] = crc_table();

/// CRC-32 (PNG/zlib).
pub(crate) fn crc32(parts: &[&[u8]]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for part in parts {
        for &byte in *part {
            c = CRC_TABLE[((c ^ u32::from(byte)) & 0xFF) as usize] ^ (c >> 8);
        }
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &byte in chunk {
            a += u32::from(byte);
            b += a;
        }
        a %= 65_521;
        b %= 65_521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(u32::MAX).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
}

/// Koduje RGBA do PNG (filtr 0, deflate w blokach „stored”).
pub(crate) fn encode_png(image: &Rgba) -> Vec<u8> {
    let row = image.width as usize * 4;
    let mut raw = Vec::with_capacity((row + 1) * image.height as usize);
    for line in image.pixels.chunks_exact(row.max(1)) {
        raw.push(0);
        raw.extend_from_slice(line);
    }
    let mut zlib = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, block) in blocks.iter().enumerate() {
        let len = u16::try_from(block.len()).unwrap_or(u16::MAX);
        zlib.push(u8::from(i + 1 == blocks.len()));
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
    }
    if blocks.is_empty() {
        zlib.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&image.width.to_be_bytes());
    ihdr.extend_from_slice(&image.height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
