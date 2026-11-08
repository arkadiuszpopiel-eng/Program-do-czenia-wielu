//! Minimalny kodek WAV (RIFF): PCM 16/24/32 bit i IEEE float 32, mono/stereo.
//! Używany przez atrapę (odtwarzanie nagrań jako mikrofon), STT (wysyłka wypowiedzi do sidecara)
//! i testy. Nieznane bloki (LIST, fact…) są pomijane.

use crate::frame::AudioFormat;
use crate::types::AudioError;

/// Kodowanie próbek w pliku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavEncoding {
    /// PCM 16 bit (np. dla whisper-server).
    Pcm16,
    /// IEEE float 32 bit (bezstratnie dla `f32`).
    Float32,
}

/// Koduje próbki przeplatane do pliku WAV.
pub fn encode_wav(samples: &[f32], format: AudioFormat, encoding: WavEncoding) -> Vec<u8> {
    let (fmt_tag, bits): (u16, u16) = match encoding {
        WavEncoding::Pcm16 => (1, 16),
        WavEncoding::Float32 => (3, 32),
    };
    let block_align = format.channels * bits / 8;
    let data_len = samples.len() * usize::from(bits / 8);
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(
        &u32::try_from(36 + data_len)
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&fmt_tag.to_le_bytes());
    out.extend_from_slice(&format.channels.to_le_bytes());
    out.extend_from_slice(&format.sample_rate.to_le_bytes());
    out.extend_from_slice(&(format.sample_rate * u32::from(block_align)).to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&u32::try_from(data_len).unwrap_or(u32::MAX).to_le_bytes());
    for &s in samples {
        match encoding {
            WavEncoding::Pcm16 => {
                let v = (s.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
                out.extend_from_slice(&v.to_le_bytes());
            }
            WavEncoding::Float32 => out.extend_from_slice(&s.to_le_bytes()),
        }
    }
    out
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

fn bad(msg: &str) -> AudioError {
    AudioError::Format(format!("WAV: {msg}"))
}

/// Dekoduje plik WAV do próbek `f32` (przeplatanych) i formatu. Format nie jest walidowany
/// względem `SUPPORTED_RATES` (resampling robi wywołujący).
pub fn decode_wav(bytes: &[u8]) -> Result<(Vec<f32>, AudioFormat), AudioError> {
    if bytes.get(0..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err(bad("brak nagłówka RIFF/WAVE"));
    }
    let mut pos = 12;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let len = u32_at(bytes, pos + 4).ok_or_else(|| bad("ucięty blok"))? as usize;
        let body = pos + 8;
        let end = body.checked_add(len).ok_or_else(|| bad("rozmiar bloku"))?;
        if id == b"fmt " {
            let tag = u16_at(bytes, body).ok_or_else(|| bad("ucięty fmt"))?;
            let channels = u16_at(bytes, body + 2).ok_or_else(|| bad("ucięty fmt"))?;
            let rate = u32_at(bytes, body + 4).ok_or_else(|| bad("ucięty fmt"))?;
            let bits = u16_at(bytes, body + 14).ok_or_else(|| bad("ucięty fmt"))?;
            // WAVE_FORMAT_EXTENSIBLE: podformat w bajtach 24..26 rozszerzenia.
            let tag = if tag == 0xFFFE {
                u16_at(bytes, body + 24).unwrap_or(1)
            } else {
                tag
            };
            fmt = Some((tag, channels, rate, bits));
        } else if id == b"data" {
            let (tag, channels, rate, bits) = fmt.ok_or_else(|| bad("blok data przed fmt"))?;
            if channels == 0 {
                return Err(bad("zero kanałów"));
            }
            let data = bytes
                .get(body..end.min(bytes.len()))
                .ok_or_else(|| bad("ucięte dane"))?;
            let samples = decode_samples(data, tag, bits)?;
            return Ok((
                samples,
                AudioFormat {
                    sample_rate: rate,
                    channels,
                },
            ));
        }
        pos = end + (len & 1);
    }
    Err(bad("brak bloku data"))
}

fn decode_samples(data: &[u8], tag: u16, bits: u16) -> Result<Vec<f32>, AudioError> {
    match (tag, bits) {
        (1, 16) => Ok(data
            .chunks_exact(2)
            .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32_768.0)
            .collect()),
        (1, 24) => Ok(data
            .chunks_exact(3)
            .map(|c| (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) as f32 / 8_388_608.0)
            .collect()),
        (1, 32) => Ok(data
            .chunks_exact(4)
            .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2_147_483_648.0)
            .collect()),
        (3, 32) => Ok(data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()),
        _ => Err(bad(&format!(
            "nieobsługiwane kodowanie (tag {tag}, {bits} bit)"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::sine;

    #[test]
    fn roundtrip_float_and_pcm16() {
        let x = sine(440.0, 16_000, 0.05, 0.7);
        let f = AudioFormat::mono(16_000);
        let (y, g) = decode_wav(&encode_wav(&x, f, WavEncoding::Float32)).unwrap();
        assert_eq!((y, g), (x.clone(), f));
        let (z, _) = decode_wav(&encode_wav(&x, f, WavEncoding::Pcm16)).unwrap();
        assert!(x.iter().zip(&z).all(|(a, b)| (a - b).abs() < 1e-4));
        let stereo = encode_wav(
            &[0.5, -0.5],
            AudioFormat::stereo(48_000),
            WavEncoding::Pcm16,
        );
        assert_eq!(decode_wav(&stereo).unwrap().1.channels, 2);
    }

    #[test]
    fn skips_unknown_chunks_and_rejects_garbage() {
        let mut w = encode_wav(&[0.25; 4], AudioFormat::mono(8_000), WavEncoding::Float32);
        // Wstaw blok LIST (nieparzysta długość → bajt wyrównania) przed `data`.
        let list = [b"LIST".as_slice(), &3u32.to_le_bytes(), b"abc", &[0]].concat();
        w.splice(36..36, list);
        let (s, _) = decode_wav(&w).unwrap();
        assert_eq!(s, vec![0.25; 4]);
        assert!(decode_wav(b"nie wav").is_err());
        let mut no_data = encode_wav(&[], AudioFormat::mono(8_000), WavEncoding::Pcm16);
        no_data.truncate(36);
        assert!(decode_wav(&no_data).is_err());
        let mut pcm8 = encode_wav(&[0.0; 2], AudioFormat::mono(8_000), WavEncoding::Pcm16);
        pcm8[34] = 8;
        assert!(decode_wav(&pcm8).is_err());
    }

    #[test]
    fn decodes_24_and_32_bit_pcm() {
        let mut w = encode_wav(&[0.0; 2], AudioFormat::mono(16_000), WavEncoding::Float32);
        w[20] = 1; // tag PCM, 32 bit
        w[44..48].copy_from_slice(&(1i32 << 30).to_le_bytes());
        let (s, _) = decode_wav(&w).unwrap();
        assert!((s[0] - 0.5).abs() < 1e-6);
        let mut w24 = encode_wav(&[0.0; 3], AudioFormat::mono(16_000), WavEncoding::Pcm16);
        w24[34] = 24;
        w24[44..47].copy_from_slice(&[0, 0, 0x40]);
        let (s, _) = decode_wav(&w24).unwrap();
        assert!((s[0] - 0.5).abs() < 1e-6);
    }
}
