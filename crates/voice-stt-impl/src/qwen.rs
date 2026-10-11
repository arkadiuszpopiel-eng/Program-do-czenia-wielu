//! [`QwenStt`]: rozpoznawanie mowy modelem **Qwen3-ASR** na `llama-server` (osobny proces na
//! `127.0.0.1`, start przy pierwszej wypowiedzi, zwolnienie po bezczynności). Nagranie idzie jako
//! WAV w zapytaniu `/v1/chat/completions`; odpowiedź ma postać `language Polish<asr_text>…`.
//! Ustawienia strojone na sprzęcie w Delcie AI (`docs/plan/19-specyfikacja-glosu-delty.md`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use base64::Engine as _;
use device_profile_contract::Backend;
use voice_audio_contract::Frame;
use voice_stt_contract::{
    Health, Stt, SttCfg, SttError, SttEvent, Transcript, UtteranceAudio, UtteranceId, check_privacy,
};

pub use crate::qwen_wire::{MODEL_NAME, QwenConfig, build_args, parse_reply, terms_prompt, wav16};
use crate::sidecar::{LaunchSpec, Sidecar, SidecarLauncher, free_port};

struct Running {
    sidecar: Box<dyn Sidecar>,
    last_used: Instant,
}

#[derive(Default)]
struct State {
    cfg: SttCfg,
    utterances: HashMap<UtteranceId, UtteranceAudio>,
    events: Vec<SttEvent>,
    health: Option<Health>,
}

/// Rozpoznawanie mowy Qwen3-ASR.
pub struct QwenStt {
    config: QwenConfig,
    launcher: Arc<dyn SidecarLauncher>,
    http: reqwest::Client,
    server: Arc<tokio::sync::Mutex<Option<Running>>>,
    state: Mutex<State>,
}

fn sidecar_err(e: impl std::fmt::Display) -> SttError {
    SttError::Sidecar(e.to_string())
}

impl QwenStt {
    /// Silnik z uruchamianiem procesu przez `launcher`.
    pub fn new(config: QwenConfig, launcher: Arc<dyn SidecarLauncher>) -> Result<Self, SttError> {
        let http = reqwest::Client::builder()
            .no_proxy()
            .timeout(config.request_timeout)
            .connect_timeout(Duration::from_secs(2))
            .build()
            .map_err(sidecar_err)?;
        Ok(Self {
            config,
            launcher,
            http,
            server: Arc::default(),
            state: Mutex::new(State::default()),
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn backend(&self) -> Backend {
        if self.config.gpu {
            Backend::Cuda
        } else {
            Backend::Cpu
        }
    }

    /// Zamyka proces, jeśli nie był używany dłużej niż `idle_unload`. Zwraca `true`, gdy zamknął.
    pub async fn unload_if_idle(&self) -> bool {
        let mut slot = self.server.lock().await;
        let idle = slot
            .as_ref()
            .is_some_and(|r| r.last_used.elapsed() >= self.config.idle_unload);
        if idle {
            if let Some(running) = slot.take() {
                running.sidecar.kill();
            }
            let mut st = self.lock();
            st.health = Some(Health::Stopped);
            st.events.push(SttEvent::ModelUnloaded {
                reason: "bezczynność".into(),
            });
        }
        idle
    }

    async fn ensure(&self, slot: &mut Option<Running>) -> Result<String, SttError> {
        if let Some(running) = slot.as_ref()
            && running.sidecar.exited().is_none()
        {
            return Ok(running.sidecar.base_url());
        }
        if let Some(dead) = slot.take() {
            dead.sidecar.kill();
        }
        for file in [&self.config.server, &self.config.model, &self.config.mmproj] {
            if !file.is_file() {
                return Err(SttError::NotAvailable(format!(
                    "brak pliku {}",
                    file.display()
                )));
            }
        }
        self.lock().health = Some(Health::Starting);
        let port = free_port()?;
        let spec = LaunchSpec {
            program: self.config.server.clone(),
            args: build_args(&self.config, port),
            port,
            backend: self.backend(),
        };
        let sidecar = self.launcher.launch(&spec).await?;
        let base = sidecar.base_url();
        let deadline = Instant::now() + self.config.startup;
        loop {
            if sidecar.exited().is_some() {
                self.lock().health =
                    Some(Health::Failed("proces zakończył się przy starcie".into()));
                return Err(SttError::Sidecar(
                    "llama-server zakończył się przy starcie".into(),
                ));
            }
            let ready = matches!(
                self.http.get(format!("{base}/health")).send().await,
                Ok(r) if r.status().is_success()
            );
            if ready {
                break;
            }
            if Instant::now() >= deadline {
                sidecar.kill();
                self.lock().health = Some(Health::Failed("przekroczony czas startu".into()));
                return Err(SttError::Sidecar(
                    "llama-server nie wystartował na czas".into(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        {
            let mut st = self.lock();
            st.health = Some(Health::Ready(self.backend()));
            st.events.push(SttEvent::ModelLoaded {
                model: MODEL_NAME.into(),
                backend: self.backend(),
            });
        }
        *slot = Some(Running {
            sidecar,
            last_used: Instant::now(),
        });
        Ok(base)
    }

    /// Rozpoznaje próbki 16 kHz mono.
    pub async fn transcribe(
        &self,
        id: UtteranceId,
        samples: &[f32],
        is_final: bool,
    ) -> Result<Transcript, SttError> {
        let cfg = self.lock().cfg.clone();
        check_privacy(&cfg)?;
        let started = Instant::now();
        let audio = base64::engine::general_purpose::STANDARD.encode(wav16(samples));
        let mut content = vec![serde_json::json!({
            "type": "input_audio",
            "input_audio": { "data": audio, "format": "wav" }
        })];
        if let Some(prompt) = terms_prompt(&cfg.hotwords) {
            content.push(serde_json::json!({ "type": "text", "text": prompt }));
        }
        let body = serde_json::json!({
            "messages": [{ "role": "user", "content": content }],
            "temperature": 0.0,
            "max_tokens": 512,
            "stream": false
        });
        let mut slot = self.server.lock().await;
        let base = self.ensure(&mut slot).await?;
        let reply = self
            .http
            .post(format!("{base}/v1/chat/completions"))
            .json(&body)
            .send()
            .await
            .map_err(sidecar_err)?;
        if !reply.status().is_success() {
            return Err(SttError::Sidecar(format!(
                "llama-server: HTTP {}",
                reply.status()
            )));
        }
        let value: serde_json::Value = reply.json().await.map_err(sidecar_err)?;
        let text = value["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| SttError::Sidecar("odpowiedź bez treści".into()))?;
        if let Some(running) = slot.as_mut() {
            running.last_used = Instant::now();
        }
        let (lang, text) = parse_reply(text);
        Ok(Transcript {
            utterance: id,
            text,
            words: Vec::new(),
            lang,
            is_final,
            // Model nie podaje pewności; wartość neutralna, a ocenę jakości robi strażnik.
            confidence: 0.8,
            latency_ms: u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX),
            backend: Some(self.backend()),
        })
    }
}

#[async_trait]
impl Stt for QwenStt {
    async fn configure(&self, cfg: SttCfg) -> Result<(), SttError> {
        check_privacy(&cfg)?;
        self.lock().cfg = cfg;
        Ok(())
    }

    async fn start_utterance(&self, id: UtteranceId) -> Result<(), SttError> {
        let mut st = self.lock();
        if st.utterances.contains_key(&id) {
            return Err(SttError::DuplicateUtterance(id));
        }
        st.utterances.insert(id, UtteranceAudio::default());
        Ok(())
    }

    async fn push(&self, id: UtteranceId, frame: &Frame) -> Result<Option<Transcript>, SttError> {
        let due = {
            let mut st = self.lock();
            let two_pass = st.cfg.two_pass;
            let min_speech = st.cfg.min_speech_ms;
            // Na procesorze rozpoznanie trwa sekundy — transkrypt w trakcie mówienia rzadziej.
            let every = if self.config.gpu {
                two_pass.partial_every_ms.max(1200)
            } else {
                two_pass.partial_every_ms.max(3000)
            };
            let audio = st
                .utterances
                .get_mut(&id)
                .ok_or(SttError::UnknownUtterance(id))?;
            audio.push(frame)?;
            (two_pass.enabled && audio.take_partial_due(every) && audio.speech_ms() >= min_speech)
                .then(|| audio.samples().to_vec())
        };
        match due {
            Some(samples) => {
                let t = self.transcribe(id, &samples, false).await?;
                self.lock().events.push(SttEvent::Partial {
                    transcript: t.clone(),
                });
                Ok(Some(t))
            }
            None => Ok(None),
        }
    }

    async fn partial_now(&self, id: UtteranceId) -> Result<Option<Transcript>, SttError> {
        let samples = {
            let st = self.lock();
            let audio = st
                .utterances
                .get(&id)
                .ok_or(SttError::UnknownUtterance(id))?;
            if audio.speech_ms() == 0 {
                return Ok(None);
            }
            audio.samples().to_vec()
        };
        self.transcribe(id, &samples, false).await.map(Some)
    }

    async fn end_utterance(&self, id: UtteranceId) -> Result<Transcript, SttError> {
        let (audio, min_speech) = {
            let mut st = self.lock();
            let audio = st
                .utterances
                .remove(&id)
                .ok_or(SttError::UnknownUtterance(id))?;
            (audio, st.cfg.min_speech_ms)
        };
        if audio.speech_ms() < min_speech {
            self.lock().events.push(SttEvent::GateRejected {
                utterance: id,
                speech_ms: audio.speech_ms(),
            });
            return Ok(Transcript::empty_final(id));
        }
        let t = self.transcribe(id, audio.samples(), true).await?;
        self.lock().events.push(SttEvent::Final {
            transcript: t.clone(),
        });
        Ok(t)
    }

    async fn cancel(&self, id: UtteranceId) {
        self.lock().utterances.remove(&id);
    }

    fn health(&self) -> Health {
        self.lock().health.clone().unwrap_or(Health::Stopped)
    }

    fn take_events(&self) -> Vec<SttEvent> {
        std::mem::take(&mut self.lock().events)
    }
}

impl Drop for QwenStt {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.server.try_lock()
            && let Some(running) = slot.take()
        {
            running.sidecar.kill();
        }
    }
}
