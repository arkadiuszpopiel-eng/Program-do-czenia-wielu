//! [`WhisperStt`]: sidecar `whisper-server` uruchamiany na żądanie, dzierżawa `model-residency`,
//! bramka VAD, dwa przebiegi (partial zachłanny / final z wiązką), fallback backendu po awarii
//! (Vulkan/CUDA → CPU) bez utraty bieżącej wypowiedzi (audio trzymane do finala).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use async_trait::async_trait;
use device_profile_contract::Backend;
use model_residency_contract::{Lease, LeaseId, LeaseListener, Residency, Revocation};
use voice_audio_contract::Frame;
use voice_stt_contract::{
    Health, Stt, SttCfg, SttEngine, SttError, SttEvent, Transcript, UtteranceAudio, UtteranceId,
    check_privacy, hotword_prompt,
};

use crate::client::{InferenceParams, WhisperClient};
use crate::sidecar::{Sidecar, SidecarLauncher, WhisperServerConfig};

#[path = "engine_start.rs"]
mod start;

/// Właściciel dzierżaw w `model-residency`.
pub const OWNER: &str = "voice-stt";

struct Server {
    sidecar: Box<dyn Sidecar>,
    backend: Backend,
    lease: Option<LeaseId>,
}

#[derive(Default)]
struct State {
    cfg: SttCfg,
    utterances: HashMap<UtteranceId, UtteranceAudio>,
    events: Vec<SttEvent>,
    failed: Vec<Backend>,
    health: Option<Health>,
    /// Czas ostatniego partiala (ms, zegar ścienny) — odstęp kolejnych to co najmniej jego
    /// dwukrotność: na CPU (kilka sekund na przebieg) partiale nie blokują mowy.
    partial_ms: u32,
}

/// Odebranie dzierżawy → sidecar do zamknięcia przy najbliższym użyciu.
struct Revoked(Arc<AtomicBool>);

impl LeaseListener for Revoked {
    fn revoked(&self, _revocation: &Revocation) {
        self.0.store(true, Ordering::SeqCst);
    }
    fn moved(&self, _lease: &Lease) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// STT na `whisper-server`.
pub struct WhisperStt {
    config: WhisperServerConfig,
    launcher: Arc<dyn SidecarLauncher>,
    client: WhisperClient,
    residency: Option<Arc<dyn Residency>>,
    preferred: Backend,
    revoked: Arc<AtomicBool>,
    server: tokio::sync::Mutex<Option<Server>>,
    state: Mutex<State>,
}

impl WhisperStt {
    /// Silnik; `preferred` — backend z rekomendacji `device-profile` (`stt_backend`).
    pub fn new(
        config: WhisperServerConfig,
        launcher: Arc<dyn SidecarLauncher>,
        preferred: Backend,
    ) -> Result<Self, SttError> {
        let client = WhisperClient::new(config.request_timeout)?;
        Ok(Self {
            config,
            launcher,
            client,
            residency: None,
            preferred,
            revoked: Arc::default(),
            server: tokio::sync::Mutex::new(None),
            state: Mutex::new(State::default()),
        })
    }

    /// Dzierżawy w `model-residency` (VRAM/RAM modelu; odebranie → zamknięcie sidecara).
    #[must_use]
    pub fn with_residency(mut self, residency: Arc<dyn Residency>) -> Self {
        residency.listen(OWNER, Arc::new(Revoked(Arc::clone(&self.revoked))));
        self.residency = Some(residency);
        self
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn emit(&self, e: SttEvent) {
        self.lock().events.push(e);
    }

    /// Zapewnia działający serwer; zwraca (adres, backend).
    async fn ensure_server(
        &self,
        slot: &mut Option<Server>,
        model: &str,
        requested: Option<Backend>,
    ) -> Result<(String, Backend), SttError> {
        if self.revoked.swap(false, Ordering::SeqCst)
            && let Some(old) = slot.take()
        {
            self.release(old);
            self.emit(SttEvent::ModelUnloaded {
                reason: "dzierżawa odebrana (rezydencja)".into(),
            });
        }
        if let Some(s) = slot.as_ref() {
            if s.sidecar.exited().is_none() {
                return Ok((s.sidecar.base_url(), s.backend));
            }
            if let Some(old) = slot.take() {
                self.release(old);
            }
        }
        let failed = self.lock().failed.clone();
        let mut backend = requested.unwrap_or(self.preferred);
        if failed.contains(&backend) || self.config.binaries.for_backend(backend).is_none() {
            backend = Backend::Cpu;
        }
        let server = self.start_with_fallback(model, backend).await?;
        let (base, backend) = (server.sidecar.base_url(), server.backend);
        {
            let mut st = self.lock();
            st.health = Some(if st.failed.is_empty() {
                Health::Ready(backend)
            } else {
                Health::Degraded(format!("{backend:?} po awarii GPU"))
            });
            st.events.push(SttEvent::ModelLoaded {
                model: model.into(),
                backend,
            });
        }
        *slot = Some(server);
        Ok((base, backend))
    }

    /// Partial (szybka wiązka) z całego dotychczasowego audio + zdarzenie `voice.stt.partial`.
    async fn partial(
        &self,
        id: UtteranceId,
        samples: &[f32],
        beam: u8,
    ) -> Result<Option<Transcript>, SttError> {
        let started = Instant::now();
        let t = self.transcribe(id, samples, beam, false).await?;
        self.lock().partial_ms = u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX);
        self.emit(SttEvent::Partial {
            transcript: t.clone(),
        });
        Ok(Some(t))
    }

    async fn transcribe(
        &self,
        id: UtteranceId,
        samples: &[f32],
        beam: u8,
        is_final: bool,
    ) -> Result<Transcript, SttError> {
        let cfg = self.lock().cfg.clone();
        let (model, requested) = match &cfg.engine {
            SttEngine::WhisperCpp { model, backend } => (model.clone(), *backend),
            SttEngine::Parakeet => {
                return Err(SttError::NotAvailable(
                    "Parakeet v3 — kandydat Voice Lab".into(),
                ));
            }
            SttEngine::Cloud { .. } => {
                check_privacy(&cfg)?;
                return Err(SttError::NotAvailable(
                    "chmurowe STT — adapter w kolejnej fali".into(),
                ));
            }
        };
        let params = InferenceParams {
            language: cfg.language.code().into(),
            beam_size: beam,
            prompt: hotword_prompt(&cfg.hotwords),
        };
        let mut slot = self.server.lock().await;
        let mut attempts = 0;
        loop {
            let (base, backend) = self.ensure_server(&mut slot, &model, requested).await?;
            match self
                .client
                .inference(&base, samples, &params, id, backend, is_final)
                .await
            {
                Ok(t) => return Ok(t),
                Err(e) => {
                    let exit = slot.as_ref().and_then(|s| s.sidecar.exited());
                    let crashed =
                        exit.is_some() || e.is_connect() || e.is_request() || e.is_timeout();
                    if !crashed || attempts >= self.config.max_restarts {
                        return Err(SttError::Sidecar(e.to_string()));
                    }
                    attempts += 1;
                    if let Some(old) = slot.take() {
                        self.release(old);
                    }
                    if backend != Backend::Cpu {
                        let lost = exit.is_some_and(|x| x.device_lost);
                        let reason = if lost {
                            "ErrorDeviceLost (GPU)"
                        } else {
                            "awaria sidecara GPU"
                        };
                        let mut st = self.lock();
                        st.failed.push(backend);
                        st.events.push(SttEvent::BackendFallback {
                            from: backend,
                            to: Backend::Cpu,
                            reason: reason.into(),
                        });
                    }
                }
            }
        }
    }
}

#[async_trait]
impl Stt for WhisperStt {
    async fn configure(&self, cfg: SttCfg) -> Result<(), SttError> {
        check_privacy(&cfg)?;
        if cfg.two_pass.partial_beam == 0 || cfg.two_pass.final_beam == 0 {
            return Err(SttError::InvalidConfig("wiązka musi być ≥ 1".into()));
        }
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
        let (due, beam) = {
            let mut st = self.lock();
            let two_pass = st.cfg.two_pass;
            let min_speech = st.cfg.min_speech_ms;
            let every = two_pass
                .partial_every_ms
                .max(st.partial_ms.saturating_mul(2));
            let audio = st
                .utterances
                .get_mut(&id)
                .ok_or(SttError::UnknownUtterance(id))?;
            audio.push(frame)?;
            let due = two_pass.enabled
                && audio.take_partial_due(every)
                && audio.speech_ms() >= min_speech;
            (due.then(|| audio.samples().to_vec()), two_pass.partial_beam)
        };
        match due {
            Some(samples) => self.partial(id, &samples, beam).await,
            None => Ok(None),
        }
    }

    async fn partial_now(&self, id: UtteranceId) -> Result<Option<Transcript>, SttError> {
        let (samples, beam) = {
            let st = self.lock();
            let audio = st
                .utterances
                .get(&id)
                .ok_or(SttError::UnknownUtterance(id))?;
            if audio.speech_ms() == 0 {
                return Ok(None);
            }
            (audio.samples().to_vec(), st.cfg.two_pass.partial_beam)
        };
        self.partial(id, &samples, beam).await
    }

    async fn end_utterance(&self, id: UtteranceId) -> Result<Transcript, SttError> {
        let (audio, min_speech, beam) = {
            let mut st = self.lock();
            let audio = st
                .utterances
                .remove(&id)
                .ok_or(SttError::UnknownUtterance(id))?;
            (audio, st.cfg.min_speech_ms, st.cfg.two_pass.final_beam)
        };
        if audio.speech_ms() < min_speech {
            self.emit(SttEvent::GateRejected {
                utterance: id,
                speech_ms: audio.speech_ms(),
            });
            return Ok(Transcript::empty_final(id));
        }
        let t = self.transcribe(id, audio.samples(), beam, true).await?;
        self.emit(SttEvent::Final {
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

impl Drop for WhisperStt {
    fn drop(&mut self) {
        if let Some(s) = self.server.get_mut().take() {
            self.release(s);
        }
    }
}
