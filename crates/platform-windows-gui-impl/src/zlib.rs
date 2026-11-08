//! Kompresja zlib dla PNG zrzutów (`flate2`, backend czysto rustowy `miniz_oxide`); przy błędzie
//! — bloki „stored” z kontraktu (poprawny, nieskompresowany PNG).

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use platform_contract::zlib_stored;

/// Strumień zlib (poziom 6 — kompromis rozmiar/czas dla zrzutów ~1–8 MP).
pub(crate) fn zlib_best(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::with_capacity(data.len() / 4), Compression::new(6));
    if encoder.write_all(data).is_err() {
        return zlib_stored(data);
    }
    encoder.finish().unwrap_or_else(|_| zlib_stored(data))
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use flate2::read::ZlibDecoder;
    use platform_contract::{RgbaImage, encode_png_with};

    use super::*;

    #[test]
    fn compresses_and_round_trips() {
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 7) as u8).collect();
        let z = zlib_best(&data);
        assert!(z.len() < data.len() / 10);
        let mut back = Vec::new();
        ZlibDecoder::new(&z[..]).read_to_end(&mut back).unwrap();
        assert_eq!(back, data);
        let img = RgbaImage::filled(640, 480, [10, 20, 30, 255]).unwrap();
        let png = encode_png_with(&img, &zlib_best);
        assert!(png.len() < 640 * 480);
    }
}
