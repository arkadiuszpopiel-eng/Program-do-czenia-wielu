//! Silnik Supertonic 3 na ONNX Runtime (procesor): przewidywanie długości → kodowanie tekstu →
//! odszumianie w `steps` krokach → wokoder. Na podstawie przykładu producenta (licencja MIT).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ort::session::Session;
use ort::value::Tensor;
use serde::Deserialize;

use crate::protocol::{Synth, SynthParams};
use crate::text;

/// Błąd silnika (tekst trafia do odpowiedzi `error`).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// Brak pliku modelu, konfiguracji albo barwy głosu.
    #[error("brak pliku: {0}")]
    Missing(String),
    /// Niepoprawna zawartość pliku.
    #[error("błąd danych: {0}")]
    Data(String),
    /// Błąd ONNX Runtime.
    #[error("ONNX Runtime: {0}")]
    Runtime(String),
    /// Tekst nie nadaje się do syntezy (pusty albo nieznany język).
    #[error("nie da się przeczytać tekstu (język: {0})")]
    Text(String),
}

impl From<ort::Error> for EngineError {
    fn from(e: ort::Error) -> Self {
        Self::Runtime(e.to_string())
    }
}

#[derive(Deserialize)]
struct Config {
    ae: AeConfig,
    ttl: TtlConfig,
}

#[derive(Deserialize)]
struct AeConfig {
    sample_rate: u32,
    base_chunk_size: usize,
}

#[derive(Deserialize)]
struct TtlConfig {
    chunk_compress_factor: usize,
    latent_dim: usize,
}

#[derive(Deserialize)]
struct StyleFile {
    style_ttl: StylePart,
    style_dp: StylePart,
}

#[derive(Deserialize)]
struct StylePart {
    data: Vec<Vec<Vec<f32>>>,
    dims: Vec<usize>,
}

/// Barwa głosu: dwa tensory stylu (dla kodera tekstu i dla przewidywania długości).
#[derive(Clone)]
struct Style {
    ttl: (Vec<usize>, Vec<f32>),
    dp: (Vec<usize>, Vec<f32>),
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, EngineError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|_| EngineError::Missing(path.display().to_string()))?;
    serde_json::from_str(&raw).map_err(|e| EngineError::Data(format!("{}: {e}", path.display())))
}

fn flatten(part: StylePart) -> Result<(Vec<usize>, Vec<f32>), EngineError> {
    let flat: Vec<f32> = part.data.into_iter().flatten().flatten().collect();
    if part.dims.len() != 3 || part.dims.iter().product::<usize>() != flat.len() {
        return Err(EngineError::Data("wymiary barwy głosu".into()));
    }
    Ok((part.dims, flat))
}

/// Szum o rozkładzie normalnym (xorshift64* + Box–Muller) — bez dodatkowych zależności.
struct Noise(u64);

impl Noise {
    fn seeded() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9E37_79B9_7F4A_7C15, |d| d.as_nanos() as u64);
        Self(nanos | 1)
    }

    fn uniform(&mut self) -> f32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let bits = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40;
        #[allow(clippy::cast_precision_loss)]
        let u = (bits as f32 + 1.0) / 16_777_217.0;
        u
    }

    fn normal(&mut self) -> f32 {
        let (u1, u2) = (self.uniform(), self.uniform());
        (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
    }
}

/// Wczytany silnik: cztery sesje ONNX, tabela znaków i barwy głosów ładowane przy pierwszym użyciu.
pub struct Supertonic {
    cfg: Config,
    indexer: Vec<i64>,
    duration: Session,
    text_enc: Session,
    estimator: Session,
    vocoder: Session,
    voices_dir: PathBuf,
    voices: HashMap<String, Style>,
    noise: Noise,
}

impl Supertonic {
    /// Wczytuje modele z `onnx_dir`; barwy głosów (`F1.json`…) są w `voices_dir`.
    pub fn load(onnx_dir: &Path, voices_dir: &Path, threads: usize) -> Result<Self, EngineError> {
        let session = |name: &str| -> Result<Session, EngineError> {
            let path = onnx_dir.join(name);
            if !path.is_file() {
                return Err(EngineError::Missing(path.display().to_string()));
            }
            Ok(Session::builder()?
                .with_intra_threads(threads.clamp(1, 16))?
                .commit_from_file(path)?)
        };
        Ok(Self {
            cfg: read_json(&onnx_dir.join("tts.json"))?,
            indexer: read_json(&onnx_dir.join("unicode_indexer.json"))?,
            duration: session("duration_predictor.onnx")?,
            text_enc: session("text_encoder.onnx")?,
            estimator: session("vector_estimator.onnx")?,
            vocoder: session("vocoder.onnx")?,
            voices_dir: voices_dir.to_path_buf(),
            voices: HashMap::new(),
            noise: Noise::seeded(),
        })
    }

    fn style(&mut self, voice: &str) -> Result<Style, EngineError> {
        if let Some(style) = self.voices.get(voice) {
            return Ok(style.clone());
        }
        // Nazwa barwy to litera i cyfry — nic, co mogłoby wyjść poza katalog głosów.
        if voice.is_empty() || !voice.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(EngineError::Missing(format!("barwa głosu „{voice}”")));
        }
        let file: StyleFile = read_json(&self.voices_dir.join(format!("{voice}.json")))?;
        let style = Style {
            ttl: flatten(file.style_ttl)?,
            dp: flatten(file.style_dp)?,
        };
        self.voices.insert(voice.to_owned(), style.clone());
        Ok(style)
    }

    /// Synteza zdania: próbki mono i częstotliwość.
    pub fn speak(
        &mut self,
        text: &str,
        params: &SynthParams,
    ) -> Result<(Vec<f32>, u32), EngineError> {
        let prepared = text::prepare(text, &params.lang)
            .ok_or_else(|| EngineError::Text(params.lang.clone()))?;
        let ids = text::to_ids(&prepared, &self.indexer);
        let len = ids.len();
        let style = self.style(&params.voice)?;
        let rate = self.cfg.ae.sample_rate;

        let text_ids = Tensor::from_array(([1usize, len], ids))?;
        let text_mask = Tensor::from_array(([1usize, 1, len], vec![1.0f32; len]))?;
        let style_dp = Tensor::from_array((style.dp.0.clone(), style.dp.1.clone()))?;
        let style_ttl = Tensor::from_array((style.ttl.0.clone(), style.ttl.1.clone()))?;

        let seconds = {
            let out = self.duration.run(ort::inputs![
                "text_ids" => &text_ids,
                "style_dp" => &style_dp,
                "text_mask" => &text_mask
            ])?;
            let (_, data) = out["duration"].try_extract_tensor::<f32>()?;
            data.first().copied().unwrap_or(0.0) / params.speed
        };
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err(EngineError::Data("model nie podał długości mowy".into()));
        }

        let (emb_shape, emb) = {
            let out = self.text_enc.run(ort::inputs![
                "text_ids" => &text_ids,
                "style_ttl" => &style_ttl,
                "text_mask" => &text_mask
            ])?;
            let (shape, data) = out["text_emb"].try_extract_tensor::<f32>()?;
            (
                shape
                    .iter()
                    .map(|d| usize::try_from(*d).unwrap_or(0))
                    .collect::<Vec<_>>(),
                data.to_vec(),
            )
        };

        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let wav_len = (seconds * rate as f32) as usize;
        let chunk = self.cfg.ae.base_chunk_size * self.cfg.ttl.chunk_compress_factor;
        let latent_len = wav_len.div_ceil(chunk.max(1)).max(1);
        let latent_dim = self.cfg.ttl.latent_dim * self.cfg.ttl.chunk_compress_factor;
        let mut latent: Vec<f32> = (0..latent_dim * latent_len)
            .map(|_| self.noise.normal())
            .collect();
        let latent_shape = [1usize, latent_dim, latent_len];

        #[allow(clippy::cast_precision_loss)]
        let total = Tensor::from_array(([1usize], vec![params.steps as f32]))?;
        for step in 0..params.steps {
            #[allow(clippy::cast_precision_loss)]
            let current = Tensor::from_array(([1usize], vec![step as f32]))?;
            let noisy = Tensor::from_array((latent_shape, latent))?;
            let text_emb = Tensor::from_array((emb_shape.clone(), emb.clone()))?;
            let latent_mask =
                Tensor::from_array(([1usize, 1, latent_len], vec![1.0f32; latent_len]))?;
            let out = self.estimator.run(ort::inputs![
                "noisy_latent" => &noisy,
                "text_emb" => &text_emb,
                "style_ttl" => &style_ttl,
                "latent_mask" => &latent_mask,
                "text_mask" => &text_mask,
                "current_step" => &current,
                "total_step" => &total
            ])?;
            let (_, data) = out["denoised_latent"].try_extract_tensor::<f32>()?;
            latent = data.to_vec();
        }

        let final_latent = Tensor::from_array((latent_shape, latent))?;
        let out = self.vocoder.run(ort::inputs!["latent" => &final_latent])?;
        let (_, wav) = out["wav_tts"].try_extract_tensor::<f32>()?;
        let wav = wav[..wav_len.min(wav.len())].to_vec();
        Ok((wav, rate))
    }
}

impl Synth for Supertonic {
    fn synth(&mut self, text: &str, params: &SynthParams) -> Result<(Vec<f32>, u32), String> {
        self.speak(text, params).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_roughly_standard_normal() {
        let mut noise = Noise(0x1234_5678_9ABC_DEF1);
        let n = 20_000;
        let samples: Vec<f32> = (0..n).map(|_| noise.normal()).collect();
        #[allow(clippy::cast_precision_loss)]
        let mean = samples.iter().sum::<f32>() / n as f32;
        #[allow(clippy::cast_precision_loss)]
        let var = samples.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / n as f32;
        assert!(mean.abs() < 0.05, "średnia {mean}");
        assert!((var - 1.0).abs() < 0.08, "wariancja {var}");
        assert!(samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn style_dimensions_are_checked() {
        let ok = StylePart {
            data: vec![vec![vec![1.0, 2.0], vec![3.0, 4.0]]],
            dims: vec![1, 2, 2],
        };
        assert_eq!(
            flatten(ok).ok().map(|(d, f)| (d.len(), f.len())),
            Some((3, 4))
        );
        let bad = StylePart {
            data: vec![vec![vec![1.0]]],
            dims: vec![1, 2, 2],
        };
        assert!(flatten(bad).is_err());
    }
}
