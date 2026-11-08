//! Dźwięk: WAV (bloki RIFF), MP3 (ID3v2, nagłówek ramki, Xing/Info/VBRI albo CBR), AAC/ADTS,
//! FLAC (STREAMINFO), Ogg (Vorbis, Opus, FLAC, Speex, Theora; czas z ostatniej strony).

use crate::{
    MediaError, MediaInfo, MediaKind, Reader, be16, be32, byte, le16, le32, le64, malformed, to_ms,
};

fn audio(r: &Reader<'_>, format: &str, mime: &str, codec: &str) -> MediaInfo {
    let mut info = MediaInfo::new(MediaKind::Audio, format, mime, r.size());
    info.codecs = vec![codec.to_owned()];
    info
}

fn wav_codec(tag: u16, bits: u16) -> String {
    match tag {
        1 if bits == 8 => "pcm_u8".into(),
        1 => format!("pcm_s{bits}le"),
        3 => format!("pcm_f{bits}le"),
        2 => "adpcm_ms".into(),
        6 => "pcm_alaw".into(),
        7 => "pcm_mulaw".into(),
        0x11 => "adpcm_ima_wav".into(),
        0x55 => "mp3".into(),
        other => format!("wav_0x{other:04x}"),
    }
}

/// WAV: blok `fmt ` (z `WAVE_FORMAT_EXTENSIBLE`) i rozmiar `data` → czas.
pub(crate) fn wav(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let mut pos = 12u64;
    let mut fmt = None;
    let mut data = None;
    while pos.saturating_add(8) <= r.size() {
        r.step()?;
        let h = r.exact(pos, 8, "WAV blok")?;
        let len = u64::from(le32(&h, 4)?);
        let body = pos + 8;
        match h.get(..4) {
            Some(b"fmt ") => {
                let f = r.exact(body, (len.min(40) as usize).max(16), "WAV fmt")?;
                let mut tag = le16(&f, 0)?;
                if tag == 0xFFFE && f.len() >= 26 {
                    tag = le16(&f, 24)?;
                }
                fmt = Some((tag, le16(&f, 2)?, le32(&f, 4)?, le32(&f, 8)?, le16(&f, 14)?));
            }
            Some(b"data") => {
                data = Some(len.min(r.size().saturating_sub(body)));
                if fmt.is_some() {
                    break;
                }
            }
            _ => {}
        }
        pos = body.saturating_add(len).saturating_add(len & 1);
    }
    let (tag, channels, rate, byte_rate, bits) = fmt.ok_or(MediaError::Truncated("WAV bez fmt"))?;
    let mut info = audio(r, "wav", "audio/wav", &wav_codec(tag, bits));
    info.sample_rate = Some(rate);
    info.channels = Some(channels);
    info.bits_per_sample = Some(bits);
    info.bit_rate = Some(u64::from(byte_rate) * 8);
    info.duration_ms = data.and_then(|d| to_ms(d, u64::from(byte_rate)));
    info.partial = data.is_none();
    Ok(info)
}

/// Nagłówek ramki MPEG audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MpegHeader {
    /// 1 = MPEG-1, 2 = MPEG-2, 25 = MPEG-2.5.
    version: u8,
    layer: u8,
    bitrate_kbps: u32,
    rate: u32,
    mono: bool,
    padding: u32,
}

const BITRATES: [[u32; 15]; 5] = [
    [
        0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
    ],
    [
        0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
    ],
    [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ],
    [
        0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
    ],
    [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
];

impl MpegHeader {
    fn samples_per_frame(self) -> u32 {
        match (self.layer, self.version) {
            (1, _) => 384,
            (3, 2 | 25) => 576,
            _ => 1152,
        }
    }

    fn frame_len(self) -> u32 {
        let br = self.bitrate_kbps * 1000;
        if self.layer == 1 {
            (12 * br / self.rate + self.padding) * 4
        } else {
            self.samples_per_frame() / 8 * br / self.rate + self.padding
        }
    }

    fn xing_offset(self) -> usize {
        4 + match (self.version, self.mono) {
            (1, false) => 32,
            (1, true) | (_, false) => 17,
            (_, true) => 9,
        }
    }
}

/// Nagłówek ramki MPEG audio z 4 bajtów (`None` — to nie ramka; wolne/złe pola odrzucone).
pub(crate) fn mpeg_header(b: &[u8]) -> Option<MpegHeader> {
    let (b0, b1, b2, b3) = (*b.first()?, *b.get(1)?, *b.get(2)?, *b.get(3)?);
    if b0 != 0xFF || b1 & 0xE0 != 0xE0 {
        return None;
    }
    let version = match (b1 >> 3) & 3 {
        0 => 25,
        2 => 2,
        3 => 1,
        _ => return None,
    };
    let layer = match (b1 >> 1) & 3 {
        1 => 3,
        2 => 2,
        3 => 1,
        _ => return None,
    };
    let index = usize::from(b2 >> 4);
    if index == 0 || index == 15 {
        return None;
    }
    let table = match (version, layer) {
        (1, l) => usize::from(l - 1),
        (_, 1) => 3,
        _ => 4,
    };
    let rate = match ((b2 >> 2) & 3, version) {
        (3, _) => return None,
        (i, 1) => [44_100, 48_000, 32_000][usize::from(i)],
        (i, 2) => [22_050, 24_000, 16_000][usize::from(i)],
        (i, _) => [11_025, 12_000, 8_000][usize::from(i)],
    };
    Some(MpegHeader {
        version,
        layer,
        bitrate_kbps: BITRATES[table][index],
        rate,
        mono: b3 >> 6 == 3,
        padding: u32::from((b2 >> 1) & 1),
    })
}

fn syncsafe(b: &[u8]) -> u64 {
    b.iter()
        .take(4)
        .fold(0, |acc, x| (acc << 7) | u64::from(x & 0x7F))
}

/// MP3: pierwsza ramka po ID3v2 (potwierdzona następną), czas z Xing/Info/VBRI albo z CBR.
pub(crate) fn mp3(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let id3 = r.get(0, 10)?;
    let mut start = 0u64;
    if id3.starts_with(b"ID3") && id3.len() == 10 {
        let footer = if byte(&id3, 5)? & 0x10 != 0 { 10 } else { 0 };
        start = 10 + syncsafe(&id3[6..10]) + footer;
    }
    let window = r.get(start, 64 * 1024)?;
    let found = (0..window.len().saturating_sub(3)).find_map(|i| {
        let h = mpeg_header(&window[i..])?;
        let next = i + h.frame_len() as usize;
        let confirmed = window
            .get(next..)
            .is_none_or(|rest| rest.len() < 4 || mpeg_header(rest).is_some());
        confirmed.then_some((i, h))
    });
    let (off, h) = found.ok_or(MediaError::Unknown)?;
    let mut info = audio(r, "mp3", "audio/mpeg", "mp3");
    info.sample_rate = Some(h.rate);
    info.channels = Some(if h.mono { 1 } else { 2 });
    info.bit_rate = Some(u64::from(h.bitrate_kbps) * 1000);
    let frame = &window[off..];
    let x = h.xing_offset();
    let frames =
        if matches!(frame.get(x..x + 4), Some(b"Xing" | b"Info")) && be32(frame, x + 4)? & 1 != 0 {
            Some(be32(frame, x + 8)?)
        } else if frame.get(36..40) == Some(b"VBRI") {
            Some(be32(frame, 50)?)
        } else {
            None
        };
    info.duration_ms = match frames {
        Some(n) => to_ms(
            u64::from(n) * u64::from(h.samples_per_frame()),
            u64::from(h.rate),
        ),
        None => {
            let audio_bytes = r.size().saturating_sub(start + off as u64);
            to_ms(
                audio_bytes.saturating_mul(8),
                u64::from(h.bitrate_kbps) * 1000,
            )
        }
    };
    info.frames = frames;
    Ok(info)
}

/// Nagłówek ADTS: (częstotliwość, kanały).
pub(crate) fn adts_header(b: &[u8]) -> Option<(u32, u16)> {
    const RATES: [u32; 13] = [
        96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025,
        8_000, 7_350,
    ];
    let (b0, b1, b2, b3) = (*b.first()?, *b.get(1)?, *b.get(2)?, *b.get(3)?);
    if b0 != 0xFF || b1 & 0xF6 != 0xF0 {
        return None;
    }
    let rate = *RATES.get(usize::from((b2 >> 2) & 0x0F))?;
    let channels = u16::from((b2 & 1) << 2 | b3 >> 6);
    Some((rate, channels))
}

/// AAC w ramkach ADTS (bez czasu — wymagałby przejścia wszystkich ramek).
pub(crate) fn aac(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let h = r.exact(0, 4, "ADTS")?;
    let (rate, channels) = adts_header(&h).ok_or(MediaError::Unknown)?;
    let mut info = audio(r, "aac", "audio/aac", "aac");
    info.sample_rate = Some(rate);
    info.channels = Some(channels);
    info.partial = true;
    Ok(info)
}

fn streaminfo(info: &mut MediaInfo, s: &[u8]) -> Result<(), MediaError> {
    let rate =
        u32::from(byte(s, 10)?) << 12 | u32::from(byte(s, 11)?) << 4 | u32::from(byte(s, 12)? >> 4);
    let total = u64::from(byte(s, 13)? & 0x0F) << 32 | u64::from(be32(s, 14)?);
    info.sample_rate = Some(rate);
    info.channels = Some(u16::from((byte(s, 12)? >> 1) & 7) + 1);
    info.bits_per_sample = Some(u16::from((byte(s, 12)? & 1) << 4 | byte(s, 13)? >> 4) + 1);
    info.duration_ms = (total > 0).then(|| to_ms(total, u64::from(rate))).flatten();
    Ok(())
}

/// FLAC: STREAMINFO (musi być pierwszym blokiem metadanych).
pub(crate) fn flac(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let h = r.exact(0, 42, "FLAC STREAMINFO")?;
    if byte(&h, 4)? & 0x7F != 0 {
        return Err(malformed("FLAC: pierwszy blok to nie STREAMINFO"));
    }
    let mut info = audio(r, "flac", "audio/flac", "flac");
    streaminfo(&mut info, &h[8..])?;
    Ok(info)
}

/// Ogg: kodek z pierwszego pakietu, czas z pozycji granuli ostatniej strony tego strumienia.
pub(crate) fn ogg(r: &mut Reader<'_>) -> Result<MediaInfo, MediaError> {
    let page = r.exact(0, 27, "Ogg strona")?;
    let serial = le32(&page, 14)?;
    let body = 27 + u64::from(byte(&page, 26)?);
    let p = r.get(body, 96)?;
    let mut info = MediaInfo::new(MediaKind::Audio, "ogg", "audio/ogg", r.size());
    let (granule_rate, preskip) = if p.starts_with(b"\x01vorbis") {
        info.codecs = vec!["vorbis".into()];
        info.channels = Some(u16::from(byte(&p, 11)?));
        let rate = le32(&p, 12)?;
        info.sample_rate = Some(rate);
        (u64::from(rate), 0)
    } else if p.starts_with(b"OpusHead") {
        info.codecs = vec!["opus".into()];
        info.channels = Some(u16::from(byte(&p, 9)?));
        info.sample_rate = Some(48_000);
        (48_000, u64::from(le16(&p, 10)?))
    } else if p.starts_with(b"\x7fFLAC") && p.get(9..13) == Some(b"fLaC") {
        info.codecs = vec!["flac".into()];
        streaminfo(&mut info, p.get(17..).unwrap_or_default())?;
        (u64::from(info.sample_rate.unwrap_or(0)), 0)
    } else if p.starts_with(b"Speex   ") {
        info.codecs = vec!["speex".into()];
        let rate = le32(&p, 36)?;
        info.sample_rate = Some(rate);
        info.channels = u16::try_from(le32(&p, 48)?).ok();
        (u64::from(rate), 0)
    } else if p.starts_with(b"\x80theora") {
        info.kind = MediaKind::Video;
        info.mime = "video/ogg".into();
        info.codecs = vec!["theora".into()];
        info.width = Some(u32::from(be16(&p, 10)?) * 16);
        info.height = Some(u32::from(be16(&p, 12)?) * 16);
        info.partial = true;
        return Ok(info);
    } else {
        info.partial = true;
        return Ok(info);
    };
    info.duration_ms =
        last_granule(r, serial)?.and_then(|g| to_ms(g.saturating_sub(preskip), granule_rate));
    info.partial = info.duration_ms.is_none();
    Ok(info)
}

fn last_granule(r: &mut Reader<'_>, serial: u32) -> Result<Option<u64>, MediaError> {
    let tail_len = r.size().min(64 * 1024 + 27);
    let tail = r.get(r.size() - tail_len, tail_len as usize)?;
    let mut i = tail.len().saturating_sub(27);
    loop {
        if tail.get(i..i + 4) == Some(b"OggS") && le32(&tail, i + 14).ok() == Some(serial) {
            let g = le64(&tail, i + 6)?;
            if g != u64::MAX {
                return Ok(Some(g));
            }
        }
        if i == 0 {
            return Ok(None);
        }
        i -= 1;
    }
}
