//! [`SpeakerPlayer`] — odtwarzanie klipów na głośniku Alfy przez `voice-audio`:
//! - **kolejka z mową agentek**: klip czeka na dzierżawę `speaker` w `scheduler-lite` (posiadaczka
//!   = agentka, priorytet narracji — zwykła odpowiedź agentki i mowa użytkownika mają pierwszeństwo);
//! - tor głosu jako `Filler` (poza „usłyszanym prefiksem”, przerywalny), normalizacja głośności
//!   miksera, porcje po 100 ms z wyprzedzeniem ≤ 1 s (bez zapychania kolejki miksera);
//! - **ducking**: prośba o wywłaszczenie (mowa użytkownika, odpowiedź agentki) → natychmiast −15 dB
//!   (rampa ≤ 50 ms), po ≤ 100 ms twardy stop i zwolnienie głośnika; odebranie dzierżawy
//!   (kill-switch), anulowanie przebiegu albo [`AudioPlayer::stop_all`] → stop od razu.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_trait::async_trait;
use personas_contract::PersonaId;
use scheduler_lite_contract::{Holder, LeaseRequest, Priority, Resource, SchedulerLite};
use tokio::time::Instant;
use tools_media_contract::{AudioClip, AudioPlayer, CancellationToken, PlayError, PlayTicket};
use voice_audio_contract::{
    AudioError, AudioFormat, AudioIo, Ducking, Frame, MediaTime, OutputStream, PlaybackState,
    SUPPORTED_RATES, SourceId, StreamConfig,
};

/// Tempo i limity odtwarzacza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerConfig {
    /// Najdłuższe czekanie w kolejce mówienia.
    pub max_wait: Duration,
    /// Porcja wysyłana do miksera.
    pub chunk: Duration,
    /// Ile dźwięku może czekać w mikserze przed urządzeniem.
    pub ahead: Duration,
    /// Odstęp sprawdzania sygnałów (wywłaszczenie, anulowanie).
    pub poll: Duration,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            max_wait: Duration::from_secs(120),
            chunk: Duration::from_millis(100),
            ahead: Duration::from_secs(1),
            poll: Duration::from_millis(10),
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Odtwarzacz na głośniku Alfy.
pub struct SpeakerPlayer {
    audio: Arc<dyn AudioIo>,
    scheduler: Arc<dyn SchedulerLite>,
    config: PlayerConfig,
    next: AtomicU64,
    active: Arc<Mutex<BTreeMap<u64, CancellationToken>>>,
}

impl std::fmt::Debug for SpeakerPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpeakerPlayer")
            .field("config", &self.config)
            .field("active", &lock(&self.active).len())
            .finish_non_exhaustive()
    }
}

impl SpeakerPlayer {
    /// Odtwarzacz nad wyjściem audio i kolejką mówienia.
    pub fn new(
        audio: Arc<dyn AudioIo>,
        scheduler: Arc<dyn SchedulerLite>,
        config: PlayerConfig,
    ) -> Self {
        Self {
            audio,
            scheduler,
            config,
            // Identyfikatory wypowiedzi odtwarzacza z wysokiego zakresu (osobne wyjście, czytelne logi).
            next: AtomicU64::new(1 << 48),
            active: Arc::default(),
        }
    }

    /// Liczba klipów czekających albo grających.
    pub fn active(&self) -> usize {
        lock(&self.active).len()
    }
}

#[async_trait]
impl AudioPlayer for SpeakerPlayer {
    async fn play(
        &self,
        clip: AudioClip,
        cancel: CancellationToken,
    ) -> Result<PlayTicket, PlayError> {
        if clip.samples.is_empty()
            || !(1..=2).contains(&clip.channels)
            || !SUPPORTED_RATES.contains(&clip.sample_rate)
            || !clip
                .samples
                .len()
                .is_multiple_of(usize::from(clip.channels))
        {
            return Err(PlayError::Format(format!(
                "pusty klip albo format {} Hz × {} kan. spoza listy miksera",
                clip.sample_rate, clip.channels
            )));
        }
        let output = self
            .audio
            .open_output(None, &StreamConfig::output_default())
            .map_err(|e| PlayError::Unavailable(e.to_string()))?;
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let stop = cancel.child_token();
        lock(&self.active).insert(id, stop.clone());
        let ticket = PlayTicket {
            id,
            duration_ms: clip.duration_ms(),
            queued: self.scheduler.holder(&Resource::Speaker).is_some(),
        };
        let job = Playback {
            scheduler: self.scheduler.clone(),
            config: self.config,
            active: self.active.clone(),
            id,
        };
        tokio::spawn(job.run(output, clip, stop));
        Ok(ticket)
    }

    fn stop_all(&self) -> usize {
        let mut active = lock(&self.active);
        for token in active.values() {
            token.cancel();
        }
        let n = active.len();
        active.clear();
        n
    }
}

struct Playback {
    scheduler: Arc<dyn SchedulerLite>,
    config: PlayerConfig,
    active: Arc<Mutex<BTreeMap<u64, CancellationToken>>>,
    id: u64,
}

impl Playback {
    async fn run(self, mut out: Box<dyn OutputStream>, clip: AudioClip, stop: CancellationToken) {
        let persona = PersonaId::new(clip.agent.as_str());
        let request = LeaseRequest::new(
            Resource::Speaker,
            Holder::Persona(persona.clone()),
            Priority::Narration,
            self.config.max_wait,
        );
        let lease = tokio::select! {
            r = self.scheduler.acquire(request) => r,
            () = stop.cancelled() => {
                lock(&self.active).remove(&self.id);
                return;
            }
        };
        match lease {
            Ok(lease) => {
                self.feed(&mut *out, &clip, &stop, &|| {
                    (lease.is_revoked(), lease.preempt_requested())
                })
                .await;
                drop(lease);
            }
            Err(e) => {
                tracing::warn!(error = %e, klip = %clip.label, "odtwarzanie: głośnik niedostępny")
            }
        }
        lock(&self.active).remove(&self.id);
    }

    /// Porcje do miksera z wyprzedzeniem; sygnały sprawdzane co `poll`.
    async fn feed(
        &self,
        out: &mut dyn OutputStream,
        clip: &AudioClip,
        stop: &CancellationToken,
        lease: &(dyn Fn() -> (bool, bool) + Sync),
    ) {
        let source = SourceId::Filler(PersonaId::new(clip.agent.as_str()));
        let format = AudioFormat {
            sample_rate: clip.sample_rate,
            channels: clip.channels,
        };
        let per_chunk = format.samples_for(self.config.chunk).max(1) * usize::from(clip.channels);
        let ahead = u64::try_from(out.format().samples_for(self.config.ahead)).unwrap_or(u64::MAX);
        let (mut pos, mut ended, mut preempted_at) = (0usize, false, None::<Instant>);
        loop {
            let (revoked, preempt) = lease();
            if stop.is_cancelled() || revoked {
                let _ = out.stop_all();
                return;
            }
            if preempt && preempted_at.is_none() {
                let _ = out.duck(Ducking::default());
                preempted_at = Some(Instant::now());
            }
            if preempted_at.is_some_and(|t| t.elapsed() >= self.config.chunk) {
                let _ = out.stop_all();
                return;
            }
            let position = out.position(self.id);
            let backlog =
                position.map_or(0, |p| p.queued_samples.saturating_sub(p.rendered_samples));
            if !ended && backlog < ahead {
                let end = (pos + per_chunk).min(clip.samples.len());
                let Some(chunk) = clip.samples.get(pos..end) else {
                    return;
                };
                let sent = Frame::new(chunk.to_vec(), format, MediaTime(0))
                    .and_then(|f| out.play(&source, self.id, &f));
                match sent {
                    Ok(()) => {}
                    Err(AudioError::QueueFull) => {
                        tokio::time::sleep(self.config.poll).await;
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "odtwarzanie przerwane");
                        let _ = out.stop_all();
                        return;
                    }
                }
                pos = end;
                if pos >= clip.samples.len() {
                    ended = true;
                    let _ = out.end_utterance(self.id);
                }
                continue;
            }
            if ended
                && position.is_none_or(|p| {
                    matches!(p.state, PlaybackState::Finished | PlaybackState::Stopped)
                })
            {
                return;
            }
            tokio::time::sleep(self.config.poll).await;
        }
    }
}
