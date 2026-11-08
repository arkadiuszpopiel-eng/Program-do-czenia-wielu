//! Testy konwersji DIB → RGBA → PNG (dekoder testowy obsługuje tylko bloki „stored”).

use super::*;

fn dib(width: i32, height: i32, bpp: u16, compression: u32, pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&bpp.to_le_bytes());
    out.extend_from_slice(&compression.to_le_bytes());
    out.extend_from_slice(&[0u8; 20]);
    if compression == 3 {
        for mask in [0x00FF_0000u32, 0x0000_FF00, 0x0000_00FF] {
            out.extend_from_slice(&mask.to_le_bytes());
        }
    }
    out.extend_from_slice(pixels);
    out
}

#[test]
fn decodes_bottom_up_32bpp_bgra() {
    // Wiersz dolny (pierwszy w pamięci): niebieski, zielony; górny: czerwony, biały. Alfa = 0 wszędzie.
    let px = [
        255, 0, 0, 0, 0, 255, 0, 0, //
        0, 0, 255, 0, 255, 255, 255, 0,
    ];
    let img = dib_to_rgba(&dib(2, 2, 32, 0, &px)).unwrap();
    assert_eq!((img.width, img.height), (2, 2));
    assert_eq!(&img.pixels[0..4], &[255, 0, 0, 255]);
    assert_eq!(&img.pixels[4..8], &[255, 255, 255, 255]);
    assert_eq!(&img.pixels[8..12], &[0, 0, 255, 255]);
    assert_eq!(&img.pixels[12..16], &[0, 255, 0, 255]);
}

#[test]
fn decodes_top_down_24bpp_with_padding_and_bitfields() {
    // Szerokość 1 przy 24 bpp → wiersz 3 B + 1 B wyrównania.
    let px = [10, 20, 30, 0, 40, 50, 60, 0];
    let img = dib_to_rgba(&dib(1, -2, 24, 0, &px)).unwrap();
    assert_eq!(img.pixels, vec![30, 20, 10, 255, 60, 50, 40, 255]);
    let px32 = [1, 2, 3, 128];
    let img = dib_to_rgba(&dib(1, 1, 32, 3, &px32)).unwrap();
    assert_eq!(img.pixels, vec![3, 2, 1, 128]);
}

#[test]
fn rejects_unsupported_or_broken_dibs() {
    assert!(dib_to_rgba(&[0u8; 10]).is_err());
    assert!(dib_to_rgba(&dib(2, 2, 8, 0, &[0; 16])).is_err());
    assert!(dib_to_rgba(&dib(2, 2, 32, 1, &[0; 16])).is_err());
    assert!(dib_to_rgba(&dib(2, 2, 32, 0, &[0; 8])).is_err());
    assert!(dib_to_rgba(&dib(0, 2, 32, 0, &[])).is_err());
    assert!(dib_to_rgba(&dib(100_000, 100_000, 32, 0, &[])).is_err());
}

fn inflate_stored(zlib: &[u8]) -> Vec<u8> {
    assert_eq!(&zlib[..2], &[0x78, 0x01]);
    let mut out = Vec::new();
    let mut at = 2;
    loop {
        let last = zlib[at] & 1 == 1;
        assert_eq!(zlib[at] >> 1, 0, "tylko bloki stored");
        let len = u16::from_le_bytes([zlib[at + 1], zlib[at + 2]]) as usize;
        let nlen = u16::from_le_bytes([zlib[at + 3], zlib[at + 4]]);
        assert_eq!(!nlen as usize, len);
        out.extend_from_slice(&zlib[at + 5..at + 5 + len]);
        at += 5 + len;
        if last {
            break;
        }
    }
    let adler = u32::from_be_bytes(zlib[at..at + 4].try_into().unwrap());
    assert_eq!(adler, adler32(&out));
    out
}

fn decode_png(png: &[u8]) -> (u32, u32, Vec<u8>) {
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let (mut at, mut ihdr, mut idat, mut ended) = (8, Vec::new(), Vec::new(), false);
    while at < png.len() {
        let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
        let kind = &png[at + 4..at + 8];
        let data = &png[at + 8..at + 8 + len];
        let crc = u32::from_be_bytes(png[at + 8 + len..at + 12 + len].try_into().unwrap());
        assert_eq!(crc, crc32(&[kind, data]));
        match kind {
            b"IHDR" => ihdr = data.to_vec(),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => ended = true,
            _ => panic!("nieoczekiwany chunk"),
        }
        at += 12 + len;
    }
    assert!(ended);
    let w = u32::from_be_bytes(ihdr[0..4].try_into().unwrap());
    let h = u32::from_be_bytes(ihdr[4..8].try_into().unwrap());
    assert_eq!(&ihdr[8..], &[8, 6, 0, 0, 0]);
    (w, h, inflate_stored(&idat))
}

#[test]
fn png_round_trip_small_and_multi_block() {
    assert_eq!(crc32(&[b"IEND"]), 0xAE42_6082);
    for (w, h) in [(2u32, 2u32), (200, 100), (1, 1)] {
        let pixels: Vec<u8> = (0..w * h * 4).map(|i| (i % 251) as u8).collect();
        let img = Rgba {
            width: w,
            height: h,
            pixels: pixels.clone(),
        };
        let (dw, dh, raw) = decode_png(&encode_png(&img));
        assert_eq!((dw, dh), (w, h));
        let row = w as usize * 4;
        let mut rebuilt = Vec::new();
        for line in raw.chunks_exact(row + 1) {
            assert_eq!(line[0], 0);
            rebuilt.extend_from_slice(&line[1..]);
        }
        assert_eq!(rebuilt, pixels);
    }
}
