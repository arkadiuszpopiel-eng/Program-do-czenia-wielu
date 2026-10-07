//! Obraz RGBA 8 bit dla zrzutów: maskowanie prostokątów, wykrywanie czarnej klatki (okno
//! chronione przed przechwyceniem, PLAN §7.3), skalowanie (średnia z obszaru) i kodowanie PNG
//! z wymiennym kompresorem zlib (implementacja Windows — `flate2`; domyślnie bloki „stored”).

use crate::gui::ScreenRect;

/// Kolor maski (ciemnoszary, nieprzezroczysty).
pub const MASK_COLOR: [u8; 4] = [24, 24, 28, 255];
/// Maksymalna liczba pikseli obrazu (ochrona pamięci; 8K × 8K).
pub const MAX_IMAGE_PIXELS: u64 = 64 * 1024 * 1024;

/// Obraz RGBA (wiersze od góry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RgbaImage {
    /// Szerokość.
    pub width: u32,
    /// Wysokość.
    pub height: u32,
    /// Piksele (`width * height * 4`).
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    /// Obraz wypełniony kolorem (`None` przy przekroczeniu [`MAX_IMAGE_PIXELS`] albo rozmiarze 0).
    pub fn filled(width: u32, height: u32, color: [u8; 4]) -> Option<Self> {
        let n = u64::from(width) * u64::from(height);
        if n == 0 || n > MAX_IMAGE_PIXELS {
            return None;
        }
        let pixels = color.repeat(usize::try_from(n).ok()?);
        Some(Self {
            width,
            height,
            pixels,
        })
    }

    /// Z bufora BGRA (GDI) — zamiana kanałów, alfa = 255.
    pub fn from_bgra(width: u32, height: u32, mut bgra: Vec<u8>) -> Option<Self> {
        let n = u64::from(width) * u64::from(height);
        if n == 0 || n > MAX_IMAGE_PIXELS || bgra.len() as u64 != n * 4 {
            return None;
        }
        for p in bgra.chunks_exact_mut(4) {
            p.swap(0, 2);
            p[3] = 255;
        }
        Some(Self {
            width,
            height,
            pixels: bgra,
        })
    }

    /// Piksel (x, y).
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        self.pixels.get(i..i + 4)?.try_into().ok()
    }

    /// Wypełnia prostokąt (współrzędne obrazu; przycinany do obrazu).
    pub fn fill_rect(&mut self, rect: &ScreenRect, color: [u8; 4]) {
        let w = i32::try_from(self.width).unwrap_or(i32::MAX);
        let h = i32::try_from(self.height).unwrap_or(i32::MAX);
        let Some(r) = rect.intersect(&ScreenRect::from_xywh(0, 0, w, h)) else {
            return;
        };
        for y in r.top..r.bottom {
            let row = y as usize * self.width as usize;
            for x in r.left..r.right {
                let i = (row + x as usize) * 4;
                if let Some(p) = self.pixels.get_mut(i..i + 4) {
                    p.copy_from_slice(&color);
                }
            }
        }
    }

    /// Czy cały obraz jest (prawie) czarny — typowy wynik przechwycenia okna chronionego
    /// (`WDA_EXCLUDEFROMCAPTURE`, DRM, okno GPU bez `PW_RENDERFULLCONTENT`).
    pub fn is_black(&self) -> bool {
        self.pixels
            .chunks_exact(4)
            .all(|p| p[0] <= 4 && p[1] <= 4 && p[2] <= 4)
    }

    /// Pomniejsza (średnia z obszaru) do zmieszczenia w `max_w × max_h`, z zachowaniem proporcji;
    /// nigdy nie powiększa.
    pub fn scale_to_fit(&self, max_w: u32, max_h: u32) -> RgbaImage {
        let (sw, sh) = (u64::from(self.width), u64::from(self.height));
        let (mw, mh) = (u64::from(max_w.max(1)), u64::from(max_h.max(1)));
        if sw <= mw && sh <= mh {
            return self.clone();
        }
        // Skala = min(mw/sw, mh/sh) w arytmetyce całkowitej.
        let (dw, dh) = if mw * sh <= mh * sw {
            (mw, (sh * mw / sw).max(1))
        } else {
            ((sw * mh / sh).max(1), mh)
        };
        let mut out = Vec::with_capacity(usize::try_from(dw * dh * 4).unwrap_or(0));
        for dy in 0..dh {
            let (y0, y1) = (dy * sh / dh, ((dy + 1) * sh / dh).max(dy * sh / dh + 1));
            for dx in 0..dw {
                let (x0, x1) = (dx * sw / dw, ((dx + 1) * sw / dw).max(dx * sw / dw + 1));
                let mut acc = [0u64; 4];
                for y in y0..y1.min(sh) {
                    let row = (y * sw) as usize;
                    for x in x0..x1.min(sw) {
                        let i = (row + x as usize) * 4;
                        for (a, v) in acc.iter_mut().zip(&self.pixels[i..i + 4]) {
                            *a += u64::from(*v);
                        }
                    }
                }
                let n = ((y1.min(sh) - y0) * (x1.min(sw) - x0)).max(1);
                out.extend(acc.iter().map(|a| u8::try_from(a / n).unwrap_or(u8::MAX)));
            }
        }
        RgbaImage {
            width: u32::try_from(dw).unwrap_or(1),
            height: u32::try_from(dh).unwrap_or(1),
            pixels: out,
        }
    }
}

fn crc32(chunks: &[&[u8]]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for data in chunks {
        for &b in *data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5_552) {
        for &x in chunk {
            a += u32::from(x);
            b += a;
        }
        a %= 65_521;
        b %= 65_521;
    }
    (b << 16) | a
}

/// Strumień zlib bez kompresji (bloki „stored”) — poprawny, ale duży; domyślny kompresor PNG.
pub fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut blocks = data.chunks(65_535).peekable();
    if blocks.peek().is_none() {
        out.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(block) = blocks.next() {
        let len = u16::try_from(block.len()).unwrap_or(u16::MAX);
        out.push(u8::from(blocks.peek().is_none()));
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&u32::try_from(data.len()).unwrap_or(0).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
}

/// Koduje PNG (RGBA 8 bit, filtr 0) z podanym kompresorem zlib.
pub fn encode_png_with(image: &RgbaImage, zlib: &dyn Fn(&[u8]) -> Vec<u8>) -> Vec<u8> {
    let stride = image.width as usize * 4;
    let mut raw = Vec::with_capacity((stride + 1) * image.height as usize);
    for row in image.pixels.chunks_exact(stride.max(1)) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&image.width.to_be_bytes());
    ihdr.extend_from_slice(&image.height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Koduje PNG bez kompresji.
pub fn encode_png(image: &RgbaImage) -> Vec<u8> {
    encode_png_with(image, &zlib_stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_reference_values() {
        assert_eq!(crc32(&[b"IEND"]), 0xAE42_6082);
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        let z = zlib_stored(b"abc");
        assert_eq!(&z[..2], &[0x78, 0x01]);
        assert_eq!(z.len(), 2 + 5 + 3 + 4);
        assert_eq!(zlib_stored(&[]).len(), 2 + 5 + 4);
        assert_eq!(
            zlib_stored(&vec![7u8; 70_000]).len(),
            2 + 2 * 5 + 70_000 + 4
        );
    }

    #[test]
    fn png_layout() {
        let img = RgbaImage::filled(3, 2, [1, 2, 3, 255]).unwrap();
        let png = encode_png(&img);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 3);
        assert_eq!(&png[png.len() - 8..png.len() - 4], b"IEND");
        assert!(RgbaImage::filled(0, 5, [0; 4]).is_none());
    }

    #[test]
    fn mask_black_and_scale() {
        let mut img = RgbaImage::filled(10, 10, [200, 0, 0, 255]).unwrap();
        assert!(!img.is_black());
        img.fill_rect(&ScreenRect::from_xywh(8, 8, 50, 50), MASK_COLOR);
        assert_eq!(img.pixel(9, 9), Some(MASK_COLOR));
        assert_eq!(img.pixel(7, 7), Some([200, 0, 0, 255]));
        assert_eq!(img.pixel(10, 0), None);
        img.fill_rect(&ScreenRect::from_xywh(-5, -5, 3, 3), MASK_COLOR);
        let small = img.scale_to_fit(5, 100);
        assert_eq!((small.width, small.height), (5, 5));
        assert_eq!(small.pixel(0, 0), Some([200, 0, 0, 255]));
        assert_eq!(small.pixel(4, 4), Some(MASK_COLOR));
        assert_eq!(img.scale_to_fit(100, 100), img);
        let wide = RgbaImage::filled(4000, 10, [0, 0, 0, 255])
            .unwrap()
            .scale_to_fit(100, 100);
        assert_eq!((wide.width, wide.height), (100, 1));
        assert!(wide.is_black());
        let bgra = RgbaImage::from_bgra(1, 1, vec![1, 2, 3, 0]).unwrap();
        assert_eq!(bgra.pixel(0, 0), Some([3, 2, 1, 255]));
        assert!(RgbaImage::from_bgra(2, 1, vec![0; 4]).is_none());
    }
}
