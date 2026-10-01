//! Audio pozycji: wczytanie WAV (mono 16 kHz, przycięcie do segmentu), sprawdzenie formatu
//! i próbki syntetyczne (CI).

use std::path::Path;

use voice_audio_contract::synth::white_noise;
use voice_audio_contract::wav::{WavEncoding, decode_wav, encode_wav};
use voice_audio_contract::{AudioFormat, PIPELINE_RATE, Resampler, downmix};

use crate::eval::manifest::{ManifestEntry, SynthSpec};

/// Częstotliwość plików w manifeście: 16 kHz (katalog `16k/` korpusu — wejście STT/VAD;
/// mastery 48 kHz zostają w `raw/`).
pub const CORPUS_RATE: u32 = 16_000;

fn read_wav(entry: &ManifestEntry, root: &Path) -> Result<(Vec<f32>, AudioFormat), String> {
    let path = root.join(&entry.audio);
    let bytes =
        std::fs::read(&path).map_err(|e| format!("{}: {} ({e})", entry.id, path.display()))?;
    decode_wav(&bytes).map_err(|e| format!("{}: {e}", entry.id))
}

/// Sprawdza plik pozycji: WAV mono 16 kHz, niepusty, dłuższy niż segment.
pub fn check_audio(entry: &ManifestEntry, root: &Path) -> Result<(), String> {
    let (pcm, format) = read_wav(entry, root)?;
    if format.channels != 1 || format.sample_rate != CORPUS_RATE {
        return Err(format!(
            "{}: wymagane mono 16 kHz (jest {} Hz × {})",
            entry.id, format.sample_rate, format.channels
        ));
    }
    let ms = pcm.len() as u64 * 1_000 / u64::from(format.sample_rate);
    if ms == 0 || entry.segment.is_some_and(|s| s.end_ms > ms) {
        return Err(format!("{}: plik ({ms} ms) krótszy niż segment", entry.id));
    }
    Ok(())
}

/// Audio pozycji jako mono 16 kHz (segment, jeśli podany).
pub fn load_item_audio(entry: &ManifestEntry, root: &Path) -> Result<Vec<f32>, String> {
    let (pcm, format) = read_wav(entry, root)?;
    let mono = downmix(&pcm, format.channels);
    let pcm = Resampler::convert(format.sample_rate, PIPELINE_RATE, &mono);
    Ok(match entry.segment {
        Some(s) => {
            let per = (PIPELINE_RATE / 1_000) as usize;
            let a = (s.start_ms as usize * per).min(pcm.len());
            let b = (s.end_ms as usize * per).min(pcm.len());
            pcm[a..b].to_vec()
        }
        None => pcm,
    })
}

/// Próbka syntetyczna (16 kHz): cisza, ciągła „mowa” (harmoniczne, obwiednia sylab), cisza.
pub fn synth_audio(spec: &SynthSpec) -> Vec<f32> {
    let per = (PIPELINE_RATE / 1_000) as usize;
    let total = (spec.lead_ms + spec.speech_ms + spec.tail_ms) as usize * per;
    let mut out = white_noise(spec.seed, total, 0.000_5);
    let fs = f64::from(PIPELINE_RATE);
    let f0 = 110.0 + (spec.seed % 40) as f64;
    let n = spec.speech_ms as usize * per;
    let lead = spec.lead_ms as usize * per;
    let fade = per * 20;
    for i in 0..n {
        let t = i as f64 / fs;
        let phase = 2.0 * std::f64::consts::PI * f0 * t;
        let v: f64 = (1..=6).map(|h| (h as f64 * phase).sin() / h as f64).sum();
        let syl = 0.6 + 0.4 * (2.0 * std::f64::consts::PI * 4.0 * t).sin();
        let edge = (i.min(n - 1 - i) as f64 / fade as f64).min(1.0);
        if let Some(x) = out.get_mut(lead + i) {
            *x += (0.3 * v * syl * edge) as f32;
        }
    }
    out
}

/// WAV PCM16 mono 16 kHz.
pub fn wav16(pcm: &[f32]) -> Vec<u8> {
    encode_wav(pcm, AudioFormat::mono(PIPELINE_RATE), WavEncoding::Pcm16)
}
