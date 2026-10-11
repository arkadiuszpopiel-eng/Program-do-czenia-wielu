//! Silniki: cecha [`TtsBackend`] i **Piper** — proces na zdanie (`--output_raw`: surowe PCM16 na
//! stdout; częstotliwość z `<model>.onnx.json`). Sidecary trwałe (Pocket, Supertonic): `sidecar.rs`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use voice_tts_contract::{SharedTuning, SpeechStyle, TtsEngine, TtsError, TtsTuning, VoiceRef};

/// Audio zdania z silnika (przed modyfikacją głosu).
#[derive(Debug, Clone, PartialEq)]
pub struct EngineAudio {
    /// Próbki mono.
    pub pcm: Vec<f32>,
    /// Częstotliwość.
    pub rate: u32,
    /// Natywne znaczniki słów (słowo, początek ms, koniec ms) względem początku zdania.
    pub marks: Option<Vec<(String, u32, u32)>>,
}

/// Silnik TTS (jedno zdanie na wywołanie).
#[async_trait]
pub trait TtsBackend: Send + Sync {
    /// Rodzaj silnika (klucz w łańcuchu).
    fn engine(&self) -> TtsEngine;
    /// Synteza zdania głosem bazowym (wysokość/tempo presetu nakłada serwis).
    async fn synth(
        &self,
        text: &str,
        voice: &VoiceRef,
        style: &SpeechStyle,
    ) -> Result<EngineAudio, TtsError>;

    /// Czy silnik sam ustawia tempo mowy (`style.rate` = tempo docelowe). Serwis nie rozciąga
    /// wtedy dźwięku po syntezie — tempo z silnika brzmi czyściej niż rozciąganie nagrania.
    fn native_tempo(&self) -> bool {
        false
    }
}

pub(crate) fn engine_err(e: impl std::fmt::Display) -> TtsError {
    TtsError::Engine(e.to_string())
}

/// PCM16 LE → `f32`.
pub fn pcm16_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32_768.0)
        .collect()
}

/// Uruchamianie procesu z wejściem na stdin i zebraniem stdout (Piper).
#[async_trait]
pub trait ProcessRunner: Send + Sync {
    /// Uruchamia `program` z `args`, pisze `stdin`, zwraca stdout.
    async fn run(
        &self,
        program: &Path,
        args: &[String],
        stdin: Vec<u8>,
        timeout: Duration,
    ) -> Result<Vec<u8>, String>;
}

/// Prawdziwe procesy (`tokio::process`).
#[derive(Debug, Default, Clone, Copy)]
pub struct TokioRunner;

#[async_trait]
impl ProcessRunner for TokioRunner {
    async fn run(
        &self,
        program: &Path,
        args: &[String],
        stdin: Vec<u8>,
        timeout: Duration,
    ) -> Result<Vec<u8>, String> {
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("{}: {e}", program.display()))?;
        let mut input = child.stdin.take().ok_or("brak stdin")?;
        let mut output = child.stdout.take().ok_or("brak stdout")?;
        let work = async move {
            input.write_all(&stdin).await.map_err(|e| e.to_string())?;
            drop(input);
            let mut out = Vec::new();
            output
                .read_to_end(&mut out)
                .await
                .map_err(|e| e.to_string())?;
            let status = child.wait().await.map_err(|e| e.to_string())?;
            if status.success() {
                Ok(out)
            } else {
                Err(format!("kod wyjścia {:?}", status.code()))
            }
        };
        tokio::time::timeout(timeout, work)
            .await
            .map_err(|_| "przekroczony czas".to_owned())?
    }
}

/// Piper `pl_PL` (model `<models_dir>/<mówczyni>.onnx` + `.onnx.json`).
pub struct PiperEngine {
    program: PathBuf,
    models_dir: PathBuf,
    runner: Arc<dyn ProcessRunner>,
    timeout: Duration,
    tuning: SharedTuning,
}

impl PiperEngine {
    /// Silnik Piper.
    pub fn new(program: PathBuf, models_dir: PathBuf, runner: Arc<dyn ProcessRunner>) -> Self {
        Self {
            program,
            models_dir,
            runner,
            timeout: Duration::from_secs(20),
            tuning: SharedTuning::default(),
        }
    }

    /// Strojenie mowy z Ustawień (ekspresja i rytm trafiają do argumentów Pipera).
    #[must_use]
    pub fn with_tuning(mut self, tuning: SharedTuning) -> Self {
        self.tuning = tuning;
        self
    }

    /// Argumenty Pipera: model, surowe PCM, tempo (`--length_scale` = 1 / tempo), ekspresja
    /// (`--noise_scale`) i zmienność rytmu (`--noise_w`).
    pub fn args(model: &Path, tuning: TtsTuning, tempo: f32) -> Vec<String> {
        let tuning = tuning.clamped();
        let length = 1.0 / tempo.clamp(0.4, 2.5);
        vec![
            "--model".into(),
            model.display().to_string(),
            "--output_raw".into(),
            "--quiet".into(),
            "--length_scale".into(),
            format!("{length:.3}"),
            "--noise_scale".into(),
            format!("{:.3}", tuning.expressiveness),
            "--noise_w".into(),
            format!("{:.3}", tuning.rhythm),
        ]
    }

    /// Częstotliwość modelu z `<model>.onnx.json` (`audio.sample_rate`, domyślnie 22 050 Hz).
    pub fn model_rate(config: &Path) -> u32 {
        std::fs::read_to_string(config)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v["audio"]["sample_rate"].as_u64())
            .and_then(|r| u32::try_from(r).ok())
            .unwrap_or(22_050)
    }
}

#[async_trait]
impl TtsBackend for PiperEngine {
    fn engine(&self) -> TtsEngine {
        TtsEngine::Piper
    }

    async fn synth(
        &self,
        text: &str,
        voice: &VoiceRef,
        style: &SpeechStyle,
    ) -> Result<EngineAudio, TtsError> {
        let model = self
            .models_dir
            .join(format!("{}.onnx", voice.preset.base_speaker));
        let config = self
            .models_dir
            .join(format!("{}.onnx.json", voice.preset.base_speaker));
        let args = Self::args(&model, self.tuning.get(), style.rate);
        let line = text.replace(['\n', '\r'], " ") + "\n";
        let raw = self
            .runner
            .run(&self.program, &args, line.into_bytes(), self.timeout)
            .await
            .map_err(TtsError::Engine)?;
        if raw.len() < 2 {
            return Err(engine_err("Piper nie zwrócił audio"));
        }
        Ok(EngineAudio {
            pcm: pcm16_to_f32(&raw),
            rate: Self::model_rate(&config),
            marks: None,
        })
    }

    fn native_tempo(&self) -> bool {
        true
    }
}
