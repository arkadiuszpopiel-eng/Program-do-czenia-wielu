//! Minimalne, poprawne pliki testowe (tylko nagłówki + krótkie dane) dla testów parserów
//! i narzędzi (feature `samples`). Nie dekodują się w odtwarzaczach — wystarczą do nagłówków.

fn be32(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

fn chunk_png(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&be32(data.len() as u32));
    out.extend_from_slice(ty);
    out.extend_from_slice(data);
    out.extend_from_slice(&[0, 0, 0, 0]);
}

/// PNG `w`×`h` RGBA 8 bit (`frames` > 0 → APNG z `acTL`).
pub fn png(w: u32, h: u32, frames: u32) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&be32(w));
    ihdr.extend_from_slice(&be32(h));
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk_png(&mut out, b"IHDR", &ihdr);
    if frames > 0 {
        let mut actl = be32(frames).to_vec();
        actl.extend_from_slice(&be32(0));
        chunk_png(&mut out, b"acTL", &actl);
    }
    chunk_png(&mut out, b"IDAT", &[0x78, 0x01]);
    chunk_png(&mut out, b"IEND", &[]);
    out
}

/// JPEG `w`×`h` (APP0 + SOF0, 3 składowe).
pub fn jpeg(w: u16, h: u16) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 16];
    out.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
    out.extend_from_slice(&[0xFF, 0xC0, 0, 17, 8]);
    out.extend_from_slice(&h.to_be_bytes());
    out.extend_from_slice(&w.to_be_bytes());
    out.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    out.extend_from_slice(&[0xFF, 0xDA, 0, 2, 0xFF, 0xD9]);
    out
}

/// GIF `w`×`h` z `frames` klatkami po `delay_cs` setnych sekundy.
pub fn gif(w: u16, h: u16, frames: u16, delay_cs: u16) -> Vec<u8> {
    let mut out = b"GIF89a".to_vec();
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&[0x80, 0, 0]);
    out.extend_from_slice(&[0; 6]);
    for _ in 0..frames {
        out.extend_from_slice(&[0x21, 0xF9, 4, 0]);
        out.extend_from_slice(&delay_cs.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out.push(0x2C);
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&w.to_le_bytes());
        out.extend_from_slice(&h.to_le_bytes());
        out.extend_from_slice(&[0, 2, 2, 0x4C, 0x01, 0]);
    }
    out.push(0x3B);
    out
}

/// BMP `w`×`h` 24 bit (`BITMAPINFOHEADER`, wysokość ujemna = od góry).
pub fn bmp(w: i32, h: i32) -> Vec<u8> {
    let mut out = b"BM".to_vec();
    out.extend_from_slice(&[0; 12]);
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    out
}

/// WebP bezstratny (VP8L) `w`×`h`.
pub fn webp_lossless(w: u32, h: u32) -> Vec<u8> {
    let mut out = b"RIFF\0\0\0\0WEBPVP8L\x05\0\0\0\x2f".to_vec();
    let bits = (w - 1) & 0x3FFF | ((h - 1) & 0x3FFF) << 14;
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(&[0; 8]);
    out
}

/// WAV PCM `bits` bit: `channels` × `rate` Hz, `ms` milisekund ciszy.
pub fn wav(rate: u32, channels: u16, bits: u16, ms: u32) -> Vec<u8> {
    let block = channels * bits / 8;
    let data_len = (u64::from(rate) * u64::from(ms) / 1000) as u32 * u32::from(block);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0");
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * u32::from(block)).to_le_bytes());
    out.extend_from_slice(&block.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.resize(out.len() + data_len as usize, 0);
    out
}

/// MP3 (MPEG-1 Layer III, 128 kb/s, 44,1 kHz, stereo) z ID3v2 i nagłówkiem Xing (`frames`).
pub fn mp3(frames: u32) -> Vec<u8> {
    let mut out = b"ID3\x04\0\0\0\0\0\x0a".to_vec();
    out.extend_from_slice(&[0; 10]);
    let header = [0xFF, 0xFB, 0x90, 0x64];
    let frame_len = 417;
    let mut first = header.to_vec();
    first.resize(36, 0);
    first.extend_from_slice(b"Xing");
    first.extend_from_slice(&be32(1));
    first.extend_from_slice(&be32(frames));
    first.resize(frame_len, 0);
    out.extend_from_slice(&first);
    let mut next = header.to_vec();
    next.resize(frame_len, 0);
    out.extend_from_slice(&next);
    out
}

/// FLAC: STREAMINFO z `rate`, `channels`, 16 bit, `total` próbek.
pub fn flac(rate: u32, channels: u8, total: u64) -> Vec<u8> {
    let mut out = b"fLaC\x80\0\0\x22".to_vec();
    let mut s = vec![0u8; 34];
    s[10] = (rate >> 12) as u8;
    s[11] = (rate >> 4) as u8;
    s[12] = ((rate & 0x0F) as u8) << 4 | (channels - 1) << 1;
    s[13] = 15 << 4 | ((total >> 32) & 0x0F) as u8;
    s[14..18].copy_from_slice(&be32(total as u32));
    out.extend_from_slice(&s);
    out
}

fn ogg_page(serial: u32, granule: u64, packet: &[u8]) -> Vec<u8> {
    let mut out = b"OggS\0\x02".to_vec();
    out.extend_from_slice(&granule.to_le_bytes());
    out.extend_from_slice(&serial.to_le_bytes());
    out.extend_from_slice(&[0; 8]);
    out.push(1);
    out.push(packet.len() as u8);
    out.extend_from_slice(packet);
    out
}

/// Ogg Opus (stereo) trwający `ms` milisekund (granula ostatniej strony, pre-skip 312).
pub fn ogg_opus(ms: u64) -> Vec<u8> {
    let mut head = b"OpusHead\x01\x02".to_vec();
    head.extend_from_slice(&312u16.to_le_bytes());
    head.extend_from_slice(&48_000u32.to_le_bytes());
    head.extend_from_slice(&[0, 0, 0]);
    let mut out = ogg_page(7, 0, &head);
    out.extend_from_slice(&ogg_page(7, 312 + ms * 48, &[0; 20]));
    out
}

fn mp4_box(ty: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = be32(8 + body.len() as u32).to_vec();
    out.extend_from_slice(ty);
    out.extend_from_slice(body);
    out
}

fn mp4_track(handler: &[u8; 4], codec: &[u8; 4], w: u32, h: u32) -> Vec<u8> {
    let mut tkhd = vec![0u8; 84];
    tkhd[76..80].copy_from_slice(&be32(w << 16));
    tkhd[80..84].copy_from_slice(&be32(h << 16));
    let mut hdlr = vec![0u8; 8];
    hdlr.extend_from_slice(handler);
    hdlr.extend_from_slice(&[0; 13]);
    let mut entry = vec![0u8; 36];
    entry[4..8].copy_from_slice(codec);
    if handler == b"soun" {
        entry[24..26].copy_from_slice(&2u16.to_be_bytes());
        entry[26..28].copy_from_slice(&16u16.to_be_bytes());
        entry[32..36].copy_from_slice(&be32(44_100 << 16));
    } else {
        entry[32..34].copy_from_slice(&(w as u16).to_be_bytes());
        entry[34..36].copy_from_slice(&(h as u16).to_be_bytes());
    }
    let entry_len = entry.len() as u32;
    entry[0..4].copy_from_slice(&be32(entry_len));
    let mut stsd = vec![0, 0, 0, 0];
    stsd.extend_from_slice(&be32(1));
    stsd.extend_from_slice(&entry);
    let stbl = mp4_box(b"stbl", &mp4_box(b"stsd", &stsd));
    let minf = mp4_box(b"minf", &stbl);
    let mut mdia = mp4_box(b"hdlr", &hdlr);
    mdia.extend_from_slice(&minf);
    let mut trak = mp4_box(b"tkhd", &tkhd);
    trak.extend_from_slice(&mp4_box(b"mdia", &mdia));
    mp4_box(b"trak", &trak)
}

/// MP4 (`isom`) z wideo H.264 `w`×`h` i dźwiękiem AAC, czas `ms`; `moov_last` — `moov` po `mdat`.
pub fn mp4(w: u32, h: u32, ms: u32, moov_last: bool) -> Vec<u8> {
    let mut mvhd = vec![0u8; 100];
    mvhd[12..16].copy_from_slice(&be32(1000));
    mvhd[16..20].copy_from_slice(&be32(ms));
    let mut moov = mp4_box(b"mvhd", &mvhd);
    moov.extend_from_slice(&mp4_track(b"vide", b"avc1", w, h));
    moov.extend_from_slice(&mp4_track(b"soun", b"mp4a", 0, 0));
    let moov = mp4_box(b"moov", &moov);
    let mdat = mp4_box(b"mdat", &[0; 64]);
    let mut out = mp4_box(b"ftyp", b"isom\0\0\x02\0isomiso2avc1mp41");
    if moov_last {
        out.extend_from_slice(&mdat);
        out.extend_from_slice(&moov);
    } else {
        out.extend_from_slice(&moov);
        out.extend_from_slice(&mdat);
    }
    out
}

/// M4A (`M4A `) tylko z dźwiękiem AAC, czas `ms`.
pub fn m4a(ms: u32) -> Vec<u8> {
    let mut mvhd = vec![0u8; 100];
    mvhd[12..16].copy_from_slice(&be32(1000));
    mvhd[16..20].copy_from_slice(&be32(ms));
    let mut moov = mp4_box(b"mvhd", &mvhd);
    moov.extend_from_slice(&mp4_track(b"soun", b"mp4a", 0, 0));
    let mut out = mp4_box(b"ftyp", b"M4A \0\0\0\0M4A isom");
    out.extend_from_slice(&mp4_box(b"moov", &moov));
    out
}

/// AVI `w`×`h` z `frames` klatkami po `usec` mikrosekund.
pub fn avi(w: u32, h: u32, frames: u32, usec: u32) -> Vec<u8> {
    let mut avih = vec![0u8; 56];
    avih[0..4].copy_from_slice(&usec.to_le_bytes());
    avih[16..20].copy_from_slice(&frames.to_le_bytes());
    avih[32..36].copy_from_slice(&w.to_le_bytes());
    avih[36..40].copy_from_slice(&h.to_le_bytes());
    let mut out = b"RIFF\0\0\0\0AVI LIST\0\0\0\0hdrlavih".to_vec();
    out.extend_from_slice(&56u32.to_le_bytes());
    out.extend_from_slice(&avih);
    out
}
