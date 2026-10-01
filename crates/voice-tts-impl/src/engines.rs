//! Silniki (sidecary): **Pocket TTS** — trwały proces z protokołem JSON-lines po stdio (żądanie →
//! fragmenty PCM16 base64 + opcjonalne znaczniki słów → `done`), **Piper** — proces na zdanie
//! (`--output_raw`: surowe PCM16 na stdout; częstotliwość z `<model>.onnx.json`).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use voice_tts_contract::{SpeechStyle, TtsEngine, TtsError, VoiceRef};

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
}

fn engine_err(e: impl std::fmt::Display) -> TtsError {
    TtsError::Engine(e.to_string())
}

/// PCM16 LE → `f32`.
pub fn pcm16_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32_768.0)
        .collect()
}

#[derive(Serialize)]
struct Request<'a> {
    id: u64,
    op: &'a str,
    text: &'a str,
    voice: &'a str,
    energy: f32,
    emotion: Option<&'a str>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Reply {
    Audio {
        id: u64,
        pcm16: String,
        sample_rate: u32,
    },
    Word {
        id: u64,
        word: String,
        start_ms: u32,
        end_ms: u32,
    },
    Done {
        id: u64,
    },
    Error {
        id: u64,
        message: String,
    },
}

type Reader = Box<dyn AsyncBufRead + Send + Unpin>;
type Writer = Box<dyn AsyncWrite + Send + Unpin>;

struct Channel {
    reader: Reader,
    writer: Writer,
    _child: Option<tokio::process::Child>,
}

/// Sidecar Pocket TTS PL (JSON-lines po stdio; protokół w README crate'a).
pub struct PocketSidecar {
    command: Option<(PathBuf, Vec<String>)>,
    channel: tokio::sync::Mutex<Option<Channel>>,
    next_id: AtomicU64,
}

impl PocketSidecar {
    /// Proces uruchamiany przy pierwszym użyciu (i ponownie po awarii).
    pub fn process(program: PathBuf, args: Vec<String>) -> Self {
        Self {
            command: Some((program, args)),
            channel: tokio::sync::Mutex::new(None),
            next_id: AtomicU64::new(1),
        }
    }

    /// Kanał gotowy (testy: `tokio::io::duplex`).
    pub fn with_channel(reader: Reader, writer: Writer) -> Self {
        Self {
            command: None,
            channel: tokio::sync::Mutex::new(Some(Channel {
                reader,
                writer,
                _child: None,
            })),
            next_id: AtomicU64::new(1),
        }
    }

    fn spawn(&self) -> Result<Channel, TtsError> {
        let (program, args) = self
            .command
            .as_ref()
            .ok_or_else(|| TtsError::NotAvailable("sidecar Pocket zakończony".into()))?;
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = cmd
            .spawn()
            .map_err(|e| TtsError::NotAvailable(format!("{}: {e}", program.display())))?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(engine_err("brak stdio sidecara"));
        };
        Ok(Channel {
            reader: Box::new(BufReader::new(stdout)),
            writer: Box::new(stdin),
            _child: Some(child),
        })
    }

    async fn exchange(ch: &mut Channel, line: String, id: u64) -> Result<EngineAudio, TtsError> {
        ch.writer
            .write_all(line.as_bytes())
            .await
            .map_err(engine_err)?;
        ch.writer.flush().await.map_err(engine_err)?;
        let (mut pcm, mut rate, mut marks) = (Vec::new(), 0u32, Vec::new());
        let mut buf = String::new();
        loop {
            buf.clear();
            if ch.reader.read_line(&mut buf).await.map_err(engine_err)? == 0 {
                return Err(engine_err("sidecar Pocket zamknął stdout"));
            }
            let Ok(reply) = serde_json::from_str::<Reply>(buf.trim()) else {
                continue; // logi/śmieci na stdout pomijamy
            };
            match reply {
                Reply::Audio {
                    id: r,
                    pcm16,
                    sample_rate,
                } if r == id => {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(pcm16)
                        .map_err(engine_err)?;
                    pcm.extend(pcm16_to_f32(&bytes));
                    rate = sample_rate;
                }
                Reply::Word {
                    id: r,
                    word,
                    start_ms,
                    end_ms,
                } if r == id => marks.push((word, start_ms, end_ms)),
                Reply::Done { id: r } if r == id => break,
                Reply::Error { id: r, message } if r == id => {
                    return Err(TtsError::Engine(message));
                }
                _ => {}
            }
        }
        if rate == 0 || pcm.is_empty() {
            return Err(engine_err("sidecar nie zwrócił audio"));
        }
        Ok(EngineAudio {
            pcm,
            rate,
            marks: (!marks.is_empty()).then_some(marks),
        })
    }
}

#[async_trait]
impl TtsBackend for PocketSidecar {
    fn engine(&self) -> TtsEngine {
        TtsEngine::Pocket
    }

    async fn synth(
        &self,
        text: &str,
        voice: &VoiceRef,
        style: &SpeechStyle,
    ) -> Result<EngineAudio, TtsError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = Request {
            id,
            op: "synth",
            text,
            voice: &voice.preset.base_speaker,
            energy: style.energy,
            emotion: style.emotion.as_deref(),
        };
        let line = serde_json::to_string(&req).map_err(engine_err)? + "\n";
        let mut slot = self.channel.lock().await;
        if slot.is_none() {
            *slot = Some(self.spawn()?);
        }
        let Some(ch) = slot.as_mut() else {
            return Err(engine_err("brak kanału"));
        };
        let result = Self::exchange(ch, line, id).await;
        if result.is_err() && self.command.is_some() {
            *slot = None; // proces do ponownego uruchomienia
        }
        result
    }
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
}

impl PiperEngine {
    /// Silnik Piper.
    pub fn new(program: PathBuf, models_dir: PathBuf, runner: Arc<dyn ProcessRunner>) -> Self {
        Self {
            program,
            models_dir,
            runner,
            timeout: Duration::from_secs(20),
        }
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
        _style: &SpeechStyle,
    ) -> Result<EngineAudio, TtsError> {
        let model = self
            .models_dir
            .join(format!("{}.onnx", voice.preset.base_speaker));
        let config = self
            .models_dir
            .join(format!("{}.onnx.json", voice.preset.base_speaker));
        let args = vec![
            "--model".into(),
            model.display().to_string(),
            "--output_raw".into(),
            "--quiet".into(),
        ];
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
}
