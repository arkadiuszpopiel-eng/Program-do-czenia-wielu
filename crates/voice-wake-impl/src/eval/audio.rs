//! Audio runnera: strumieniowy odczyt WAV 16 kHz mono (PCM16 albo float32) blokami — nagrania tła
//! mają godziny, więc plik nie jest wczytywany w całości — oraz próbki syntetyczne CI.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use voice_audio_contract::synth::{SpeechParams, sine, synthetic_speech, white_noise};

use super::manifest::SynthRecipe;

/// Częstotliwość wymagana przez runner.
pub const RATE: u32 = 16_000;

/// Strumieniowy czytnik WAV.
pub struct WavReader {
    reader: BufReader<File>,
    left: u64,
    float: bool,
    /// Długość (ms).
    pub duration_ms: u64,
}

fn read_exact<R: Read>(r: &mut R, n: usize) -> Result<Vec<u8>, String> {
    let mut b = vec![0u8; n];
    r.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(b)
}

fn le_u16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

impl WavReader {
    /// Otwiera plik i sprawdza format (16 kHz, mono, PCM16 albo float32).
    pub fn open(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut r = BufReader::new(file);
        let riff = read_exact(&mut r, 12)?;
        if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
            return Err(format!("{}: to nie jest plik WAV", path.display()));
        }
        let mut fmt: Option<(u16, u16, u32, u16)> = None;
        loop {
            let head =
                read_exact(&mut r, 8).map_err(|_| format!("{}: brak danych", path.display()))?;
            let size = le_u32(&head, 4) as usize;
            match &head[0..4] {
                b"fmt " => {
                    let b = read_exact(&mut r, size + size % 2)?;
                    if b.len() < 16 {
                        return Err("nagłówek fmt za krótki".into());
                    }
                    fmt = Some((le_u16(&b, 0), le_u16(&b, 2), le_u32(&b, 4), le_u16(&b, 14)));
                }
                b"data" => {
                    let (format, channels, rate, bits) =
                        fmt.ok_or_else(|| "chunk data przed fmt".to_owned())?;
                    let float = match (format, bits) {
                        (1, 16) => false,
                        (3, 32) | (0xFFFE, 32) => true,
                        (0xFFFE, 16) => false,
                        _ => {
                            return Err(format!(
                                "format {format}/{bits} bit — wymagany PCM16 albo float32"
                            ));
                        }
                    };
                    if channels != 1 || rate != RATE {
                        return Err(format!(
                            "{}: {rate} Hz × {channels} kan. — wymagane 16 kHz mono \
                             (np. `ffmpeg -i in.wav -ac 1 -ar 16000 out.wav`)",
                            path.display()
                        ));
                    }
                    let bytes_per = if float { 4 } else { 2 };
                    let samples = size as u64 / bytes_per;
                    return Ok(Self {
                        reader: r,
                        left: samples,
                        float,
                        duration_ms: samples * 1000 / u64::from(RATE),
                    });
                }
                _ => {
                    let skip = (size + size % 2) as u64;
                    std::io::copy(&mut (&mut r).take(skip), &mut std::io::sink())
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }

    /// Następny blok (≤ `max` próbek); pusty = koniec.
    pub fn next_block(&mut self, max: usize) -> Result<Vec<f32>, String> {
        let n = (max as u64).min(self.left) as usize;
        self.left -= n as u64;
        let width = if self.float { 4 } else { 2 };
        let raw = read_exact(&mut self.reader, n * width)?;
        Ok(raw
            .chunks_exact(width)
            .map(|c| {
                if self.float {
                    f32::from_le_bytes([c[0], c[1], c[2], c[3]])
                } else {
                    f32::from(i16::from_le_bytes([c[0], c[1]])) / 32_768.0
                }
            })
            .collect())
    }
}

/// Próbka syntetyczna z przepisu: cichy szum (`lead_secs`), potem ton albo mowa syntetyczna,
/// na końcu 0,5 s szumu.
pub fn synth(recipe: &SynthRecipe) -> Vec<f32> {
    let n = |s: f32| (s.max(0.0) * RATE as f32) as usize;
    let mut out = white_noise(recipe.seed, n(recipe.lead_secs), 0.001);
    let main = match recipe.kind.as_str() {
        "tone" => sine(recipe.tone_hz, RATE, recipe.secs, 0.3),
        _ => synthetic_speech(
            RATE,
            recipe.secs,
            SpeechParams {
                seed: recipe.seed,
                ..SpeechParams::default()
            },
        ),
    };
    out.extend(main);
    out.extend(white_noise(recipe.seed.wrapping_add(1), n(0.5), 0.001));
    out
}
