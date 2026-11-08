//! Kontenery wideo: ISO BMFF (MP4/MOV/M4A/3GP/HEIF — drzewo pudełek z limitem głębokości
//! i kroków, bez czytania `mdat`), Matroska/WebM (tylko rozpoznanie), AVI (`avih`).

use crate::{
    MediaError, MediaInfo, MediaKind, Reader, be16, be32, be64, byte, le32, malformed, to_ms,
};

/// Ścieżka MP4: typ, kodek i pola nagłówków.
#[derive(Debug, Default, Clone)]
struct Track {
    handler: [u8; 4],
    codec: Option<[u8; 4]>,
    tkhd_dims: (u32, u32),
    entry_dims: (u32, u32),
    channels: u16,
    sample_rate: u32,
    sample_bits: u16,
}

#[derive(Debug, Default)]
struct Movie {
    timescale: u64,
    duration: u64,
    tracks: Vec<Track>,
}

/// Nagłówek pudełka: (typ, długość nagłówka, długość całości) w granicach `limit`.
fn box_at(r: &mut Reader<'_>, pos: u64, limit: u64) -> Result<([u8; 4], u64, u64), MediaError> {
    let h = r.exact(pos, 8, "MP4 pudełko")?;
    let ty = [h[4], h[5], h[6], h[7]];
    let (hdr, len) = match be32(&h, 0)? {
        1 => (16, be64(&r.exact(pos + 8, 8, "MP4 rozmiar 64-bit")?, 0)?),
        0 => (8, limit.saturating_sub(pos)),
        n => (8, u64::from(n)),
    };
    if len < hdr || pos.checked_add(len).is_none_or(|end| end > limit) {
        return Err(malformed("MP4: rozmiar pudełka poza rodzicem"));
    }
    Ok((ty, hdr, len))
}

fn brand_format(brand: &[u8]) -> (&'static str, &'static str, MediaKind) {
    match brand {
        b"qt  " => ("mov", "video/quicktime", MediaKind::Video),
        b"M4A " | b"M4B " => ("m4a", "audio/mp4", MediaKind::Audio),
        b"M4V " => ("m4v", "video/x-m4v", MediaKind::Video),
        b"heic" | b"heix" | b"mif1" | b"msf1" => ("heif", "image/heif", MediaKind::Image),
        b"avif" | b"avis" => ("avif", "image/avif", MediaKind::Image),
        b if b.starts_with(b"3g2") => ("3g2", "video/3gpp2", MediaKind::Video),
        b if b.starts_with(b"3gp") => ("3gp", "video/3gpp", MediaKind::Video),
        _ => ("mp4", "video/mp4", MediaKind::Video),
    }
}

fn codec_name(fourcc: [u8; 4]) -> String {
    match &fourcc {
        b"avc1" | b"avc3" => "h264".into(),
        b"hvc1" | b"hev1" => "hevc".into(),
        b"vp09" => "vp9".into(),
        b"av01" => "av1".into(),
        b"mp4v" => "mpeg4".into(),
        b"mp4a" => "aac".into(),
        b"Opus" => "opus".into(),
        b"fLaC" => "flac".into(),
        b"ac-3" => "ac3".into(),
        b"ec-3" => "eac3".into(),
        b".mp3" => "mp3".into(),
        other => other
            .iter()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|&c| char::from(c))
            .collect(),
    }
}

/// MP4: `ftyp` (marka → format), `moov` (czas, ścieżki, kodeki, wymiary).
pub(crate) fn mp4(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let size = r.size();
    let (mut pos, mut brand, mut moov) = (0u64, None, None);
    while pos.saturating_add(8) <= size {
        r.step()?;
        let (ty, hdr, len) = box_at(r, pos, size)?;
        match &ty {
            b"ftyp" => brand = Some(r.exact(pos + hdr, 4, "MP4 ftyp")?),
            b"moov" => moov = Some((pos + hdr, pos + len)),
            _ => {}
        }
        pos += len;
    }
    let brand = brand.ok_or_else(|| malformed("MP4 bez ftyp"))?;
    let (format, mime, kind) = brand_format(&brand);
    let mut info = MediaInfo::new(kind, format, mime, size);
    if kind == MediaKind::Image {
        info.partial = true;
        return Ok(info);
    }
    let (start, end) = moov.ok_or(MediaError::Truncated("MP4 bez moov"))?;
    let mut movie = Movie::default();
    walk(r, start, end, 0, &mut movie)?;
    info.duration_ms = to_ms(movie.duration, movie.timescale);
    let video = movie.tracks.iter().find(|t| &t.handler == b"vide");
    let sound = movie.tracks.iter().find(|t| &t.handler == b"soun");
    if let Some(v) = video {
        let (w, h) = if v.tkhd_dims.0 > 0 {
            v.tkhd_dims
        } else {
            v.entry_dims
        };
        info.width = (w > 0).then_some(w);
        info.height = (h > 0).then_some(h);
    } else if sound.is_some() {
        info.kind = MediaKind::Audio;
        if info.mime == "video/mp4" {
            info.format = "m4a".into();
            info.mime = "audio/mp4".into();
        }
    }
    if let Some(a) = sound {
        info.sample_rate = (a.sample_rate > 0).then_some(a.sample_rate);
        info.channels = (a.channels > 0).then_some(a.channels);
        info.bits_per_sample = (a.sample_bits > 0).then_some(a.sample_bits);
    }
    info.codecs = movie
        .tracks
        .iter()
        .filter_map(|t| t.codec.map(codec_name))
        .collect();
    info.partial = movie.tracks.is_empty();
    Ok(info)
}

fn walk(
    r: &mut Reader<'_>,
    start: u64,
    end: u64,
    depth: u32,
    movie: &mut Movie,
) -> Result<(), MediaError> {
    if depth > r.limits.max_depth {
        return Err(MediaError::Limit(
            "zbyt głębokie zagnieżdżenie pudełek".into(),
        ));
    }
    let mut pos = start;
    while pos.saturating_add(8) <= end {
        r.step()?;
        let (ty, hdr, len) = box_at(r, pos, end)?;
        let (body, body_end) = (pos + hdr, pos + len);
        match &ty {
            b"mvhd" => {
                let (timescale, duration) = times(r, body)?;
                movie.timescale = timescale;
                movie.duration = duration;
            }
            b"trak" => {
                movie.tracks.push(Track::default());
                walk(r, body, body_end, depth + 1, movie)?;
            }
            b"mdia" | b"minf" | b"stbl" => walk(r, body, body_end, depth + 1, movie)?,
            b"tkhd" => {
                let v = byte(&r.exact(body, 1, "MP4 tkhd")?, 0)?;
                let at = if v == 1 { 88 } else { 76 };
                let d = r.exact(body + at, 8, "MP4 tkhd wymiary")?;
                if let Some(t) = movie.tracks.last_mut() {
                    t.tkhd_dims = (be32(&d, 0)? >> 16, be32(&d, 4)? >> 16);
                }
            }
            b"hdlr" => {
                let d = r.exact(body + 8, 4, "MP4 hdlr")?;
                if let Some(t) = movie.tracks.last_mut() {
                    t.handler = [d[0], d[1], d[2], d[3]];
                }
            }
            b"stsd" => stsd(r, body, movie)?,
            _ => {}
        }
        pos = body_end;
    }
    Ok(())
}

/// `mvhd`/`mdhd`: (skala czasu, czas trwania); wersja 1 — pola 64-bitowe.
fn times(r: &mut Reader<'_>, body: u64) -> Result<(u64, u64), MediaError> {
    let d = r.exact(body, 32, "MP4 mvhd")?;
    if byte(&d, 0)? == 1 {
        Ok((u64::from(be32(&d, 20)?), be64(&d, 24)?))
    } else {
        let duration = be32(&d, 16)?;
        let duration = if duration == u32::MAX {
            0
        } else {
            u64::from(duration)
        };
        Ok((u64::from(be32(&d, 12)?), duration))
    }
}

/// `stsd`: pierwszy wpis — kodek (fourcc) i pola wpisu dźwięku/obrazu.
fn stsd(r: &mut Reader<'_>, body: u64, movie: &mut Movie) -> Result<(), MediaError> {
    let d = r.get(body, 8 + 40)?;
    if d.len() < 16 || be32(&d, 4)? == 0 {
        return Ok(());
    }
    let e = &d[8..];
    let Some(t) = movie.tracks.last_mut() else {
        return Ok(());
    };
    t.codec = Some([e[4], e[5], e[6], e[7]]);
    if e.len() >= 36 {
        t.channels = be16(e, 24)?;
        t.sample_bits = be16(e, 26)?;
        t.sample_rate = be32(e, 32)? >> 16;
        t.entry_dims = (u32::from(be16(e, 32)?), u32::from(be16(e, 34)?));
    }
    Ok(())
}

/// Matroska/WebM: rozpoznanie (DocType); szczegóły ścieżek wymagają parsera EBML — poza zakresem.
pub(crate) fn matroska(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let head = r.get(0, 64)?;
    let webm = head.windows(4).any(|w| w == b"webm");
    let (format, mime) = if webm {
        ("webm", "video/webm")
    } else {
        ("matroska", "video/x-matroska")
    };
    let mut info = MediaInfo::new(MediaKind::Video, format, mime, r.size());
    info.partial = true;
    Ok(info)
}

/// AVI: nagłówek `avih` (czas klatki, liczba klatek, wymiary).
pub(crate) fn avi(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let head = r.get(0, 4096)?;
    let at = head
        .windows(4)
        .position(|w| w == b"avih")
        .ok_or(MediaError::Truncated("AVI bez avih"))?;
    let d = head
        .get(at + 8..at + 48)
        .ok_or(MediaError::Truncated("AVI avih"))?;
    let usec_per_frame = u64::from(le32(d, 0)?);
    let frames = le32(d, 16)?;
    let mut info = MediaInfo::new(MediaKind::Video, "avi", "video/x-msvideo", r.size());
    info.frames = Some(frames);
    info.duration_ms = to_ms(u64::from(frames) * usec_per_frame, 1_000_000);
    let (w, h) = (le32(d, 32)?, le32(d, 36)?);
    info.width = (w > 0).then_some(w);
    info.height = (h > 0).then_some(h);
    info.partial = true;
    Ok(info)
}
