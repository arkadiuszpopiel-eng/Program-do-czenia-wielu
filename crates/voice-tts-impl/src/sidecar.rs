//! Trwałe sidecary mowy z protokołem JSON-lines po stdio (żądanie → fragmenty PCM16 base64 +
//! opcjonalne znaczniki słów → `done`): **Pocket TTS** i **Supertonic 3** (`alfa-supertonic`,
//! crate `lib-supertonic`). Proces startuje przy pierwszym zdaniu i wraca po awarii.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use voice_tts_contract::{SharedTuning, SpeechStyle, TtsEngine, TtsError, VoiceRef};

use crate::engines::{EngineAudio, TtsBackend, engine_err, pcm16_to_f32};

/// Tempo naturalne Supertonic 3 (wartość strojona na sprzęcie w Delcie AI: 1,05).
pub const SUPERTONIC_BASE_RATE: f32 = 1.05;

#[derive(Serialize)]
struct Request<'a> {
    id: u64,
    op: &'a str,
    text: &'a str,
    voice: &'a str,
    energy: f32,
    emotion: Option<&'a str>,
    /// Tempo docelowe — tylko dla silników, które ustawiają je same (Supertonic).
    #[serde(skip_serializing_if = "Option::is_none")]
    rate: Option<f32>,
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

/// Trwały sidecar mowy (JSON-lines po stdio; protokół w README crate'a): Pocket TTS PL albo
/// Supertonic 3 (`alfa-supertonic`). Różnią się rodzajem silnika i tym, kto ustawia tempo.
pub struct PocketSidecar {
    engine: TtsEngine,
    /// Mnożnik tempa wysyłany do silnika (`None` = tempo nakłada serwis po syntezie).
    native_rate: Option<f32>,
    /// Strojenie z Ustawień — Supertonic bierze z niego barwę wybraną dla agentki.
    tuning: Option<SharedTuning>,
    command: Option<(PathBuf, Vec<String>)>,
    channel: tokio::sync::Mutex<Option<Channel>>,
    next_id: AtomicU64,
}

impl PocketSidecar {
    /// Proces uruchamiany przy pierwszym użyciu (i ponownie po awarii).
    pub fn process(program: PathBuf, args: Vec<String>) -> Self {
        Self {
            engine: TtsEngine::Pocket,
            native_rate: None,
            tuning: None,
            command: Some((program, args)),
            channel: tokio::sync::Mutex::new(None),
            next_id: AtomicU64::new(1),
        }
    }

    /// Kanał gotowy (testy: `tokio::io::duplex`).
    pub fn with_channel(reader: Reader, writer: Writer) -> Self {
        Self {
            engine: TtsEngine::Pocket,
            native_rate: None,
            tuning: None,
            command: None,
            channel: tokio::sync::Mutex::new(Some(Channel {
                reader,
                writer,
                _child: None,
            })),
            next_id: AtomicU64::new(1),
        }
    }

    /// Ten sam sidecar jako silnik Supertonic 3: tempo ustawia silnik (`style.rate` × tempo
    /// naturalne modelu [`SUPERTONIC_BASE_RATE`]), więc serwis nie rozciąga nagrania.
    #[must_use]
    pub fn supertonic(mut self) -> Self {
        self.engine = TtsEngine::Supertonic;
        self.native_rate = Some(SUPERTONIC_BASE_RATE);
        self
    }

    /// Strojenie z Ustawień (barwy agentek czytane przy każdym zdaniu — zmiana działa od razu).
    #[must_use]
    pub fn with_tuning(mut self, tuning: SharedTuning) -> Self {
        self.tuning = Some(tuning);
        self
    }

    /// Głos wysyłany do silnika: barwa wybrana w Ustawieniach (tylko Supertonic i agentki
    /// wbudowane) albo mówczyni z łańcucha głosów.
    fn speaker<'a>(&self, voice: &'a VoiceRef) -> &'a str {
        if self.engine == TtsEngine::Supertonic
            && let Some(tuning) = &self.tuning
            && let Some(timbre) = tuning.get().timbre_for(voice.persona.as_str())
        {
            return timbre;
        }
        &voice.preset.base_speaker
    }

    fn spawn(&self) -> Result<Channel, TtsError> {
        let (program, args) = self
            .command
            .as_ref()
            .ok_or_else(|| TtsError::NotAvailable("sidecar mowy zakończony".into()))?;
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
                return Err(engine_err("sidecar mowy zamknął stdout"));
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
        self.engine.clone()
    }

    fn native_tempo(&self) -> bool {
        self.native_rate.is_some()
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
            voice: self.speaker(voice),
            energy: style.energy,
            emotion: style.emotion.as_deref(),
            rate: self
                .native_rate
                .map(|base| (style.rate * base).clamp(0.5, 2.0)),
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
