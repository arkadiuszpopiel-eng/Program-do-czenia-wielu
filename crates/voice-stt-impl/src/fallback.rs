//! [`GuardedStt`]: silnik główny (Qwen3-ASR) ze strażnikiem i silnikiem zapasowym (Whisper).
//! Kolejność jak w Delcie AI: odrzut pustych nagrań → bezpiecznik → rozpoznanie → flagi jakości →
//! przy fladze albo awarii to samo nagranie rozpoznaje silnik zapasowy → lista zmyśleń.
//! Nagranie jest trzymane do końca wypowiedzi, więc przejście na zapas niczego nie gubi.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use voice_audio_contract::Frame;
use voice_stt_contract::{
    Health, LangMode, Stt, SttCfg, SttError, SttEvent, Transcript, UtteranceId,
};

use crate::breaker::{Breaker, BreakerState};
use crate::guard::{self, AudioStats, Flag};

/// Ile ostatnich rozpoznań pamięta telemetria.
const TELEMETRY: usize = 200;

/// Jedno rozpoznanie w telemetrii (bez treści wypowiedzi).
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// Silnik, który dał wynik (`glowny` / `zapasowy` / `brak`).
    pub engine: &'static str,
    /// Długość nagrania (ms).
    pub audio_ms: u32,
    /// Czas rozpoznania (ms).
    pub processing_ms: u32,
    /// Flagi jakości wyniku silnika głównego.
    pub flags: Vec<Flag>,
    /// Czy wynik pochodzi z silnika zapasowego po fladze albo awarii.
    pub fallback_used: bool,
    /// Nagranie odrzucone przed rozpoznaniem (cisza, brak sygnału, za krótkie).
    pub skipped: bool,
}

/// Stan do pokazania w oknie.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardStatus {
    /// Stan bezpiecznika.
    pub breaker: BreakerState,
    /// Awarie z rzędu.
    pub failures: u32,
    /// Próg otwarcia.
    pub failures_to_open: u32,
    /// Ile razy bezpiecznik się otworzył.
    pub opened_count: u32,
    /// Sekundy do następnej próby (gdy otwarty).
    pub retry_in_s: Option<u32>,
    /// Mediana i 95. centyl czasu rozpoznania (ms) z ostatnich próbek.
    pub processing_ms: Option<(u32, u32)>,
    /// Liczba próbek, przejść na zapas i wyników z flagą.
    pub counts: (usize, usize, usize),
}

struct Utterance {
    frames: Vec<Frame>,
    on_primary: bool,
}

struct State {
    cfg: SttCfg,
    utterances: HashMap<UtteranceId, Utterance>,
    events: Vec<SttEvent>,
    breaker: Breaker,
    samples: VecDeque<Sample>,
}

/// Rozpoznawanie z silnikiem zapasowym i strażnikiem jakości.
pub struct GuardedStt {
    primary: Arc<dyn Stt>,
    fallback: Arc<dyn Stt>,
    started: Instant,
    state: Mutex<State>,
}

impl GuardedStt {
    /// Para silników z bezpiecznikiem o wartościach z Delty (3 awarie, 30 s przerwy).
    pub fn new(primary: Arc<dyn Stt>, fallback: Arc<dyn Stt>) -> Self {
        Self {
            primary,
            fallback,
            started: Instant::now(),
            state: Mutex::new(State {
                cfg: SttCfg::default(),
                utterances: HashMap::new(),
                events: Vec::new(),
                breaker: Breaker::new(3, Duration::from_secs(30)),
                samples: VecDeque::with_capacity(TELEMETRY),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn now(&self) -> Duration {
        self.started.elapsed()
    }

    /// Ręczne zamknięcie bezpiecznika.
    pub fn reset_breaker(&self) {
        self.lock().breaker.reset();
    }

    /// Stan bezpiecznika i skrót telemetrii.
    pub fn status(&self) -> GuardStatus {
        let now = self.now();
        let st = self.lock();
        let (breaker, failures, failures_to_open, opened_count) = st.breaker.snapshot();
        let mut times: Vec<u32> = st
            .samples
            .iter()
            .filter(|s| !s.skipped)
            .map(|s| s.processing_ms)
            .collect();
        times.sort_unstable();
        let processing_ms = (!times.is_empty()).then(|| {
            let p95 = (times.len() * 95).div_ceil(100).saturating_sub(1);
            (times[times.len() / 2], times[p95.min(times.len() - 1)])
        });
        GuardStatus {
            breaker,
            failures,
            failures_to_open,
            opened_count,
            retry_in_s: st
                .breaker
                .retry_in(now)
                .map(|d| u32::try_from(d.as_secs()).unwrap_or(u32::MAX)),
            processing_ms,
            counts: (
                st.samples.len(),
                st.samples.iter().filter(|s| s.fallback_used).count(),
                st.samples.iter().filter(|s| !s.flags.is_empty()).count(),
            ),
        }
    }

    fn record(&self, sample: Sample) {
        let mut st = self.lock();
        if st.samples.len() == TELEMETRY {
            st.samples.pop_front();
        }
        st.samples.push_back(sample);
    }

    /// To samo nagranie od początku na silniku zapasowym.
    async fn replay(&self, id: UtteranceId, frames: &[Frame]) -> Result<Transcript, SttError> {
        self.fallback.cancel(id).await;
        self.fallback.start_utterance(id).await?;
        for frame in frames {
            // Transkrypty częściowe z powtórki są zbędne — liczy się wynik końcowy.
            let _ = self.fallback.push(id, frame).await?;
        }
        self.fallback.end_utterance(id).await
    }
}

fn requested_lang(cfg: &SttCfg) -> Option<&'static str> {
    match cfg.language {
        LangMode::Auto => None,
        other => Some(other.code()),
    }
}

fn samples_of(frames: &[Frame]) -> Vec<f32> {
    frames.iter().flat_map(|f| f.pcm.iter().copied()).collect()
}

#[async_trait]
impl Stt for GuardedStt {
    async fn configure(&self, cfg: SttCfg) -> Result<(), SttError> {
        self.primary.configure(cfg.clone()).await?;
        self.fallback.configure(cfg.clone()).await?;
        self.lock().cfg = cfg;
        Ok(())
    }

    async fn start_utterance(&self, id: UtteranceId) -> Result<(), SttError> {
        if self.lock().utterances.contains_key(&id) {
            return Err(SttError::DuplicateUtterance(id));
        }
        let now = self.now();
        let allowed = self.lock().breaker.allow(now);
        let on_primary = allowed && self.primary.start_utterance(id).await.is_ok();
        if allowed && !on_primary {
            self.lock().breaker.record(false, now);
        }
        if !on_primary {
            self.fallback.start_utterance(id).await?;
        }
        self.lock().utterances.insert(
            id,
            Utterance {
                frames: Vec::new(),
                on_primary,
            },
        );
        Ok(())
    }

    async fn push(&self, id: UtteranceId, frame: &Frame) -> Result<Option<Transcript>, SttError> {
        let on_primary = {
            let mut st = self.lock();
            let utt = st
                .utterances
                .get_mut(&id)
                .ok_or(SttError::UnknownUtterance(id))?;
            utt.frames.push(frame.clone());
            utt.on_primary
        };
        if !on_primary {
            return self.fallback.push(id, frame).await;
        }
        match self.primary.push(id, frame).await {
            Ok(partial) => Ok(partial),
            // Błąd transkryptu częściowego nie kończy wypowiedzi: nagranie jest w buforze,
            // a o silniku zdecyduje wynik końcowy.
            Err(SttError::Format(e)) => Err(SttError::Format(e)),
            Err(_) => Ok(None),
        }
    }

    async fn partial_now(&self, id: UtteranceId) -> Result<Option<Transcript>, SttError> {
        let on_primary = self
            .lock()
            .utterances
            .get(&id)
            .map(|u| u.on_primary)
            .ok_or(SttError::UnknownUtterance(id))?;
        if on_primary {
            Ok(self.primary.partial_now(id).await.unwrap_or(None))
        } else {
            self.fallback.partial_now(id).await
        }
    }

    async fn end_utterance(&self, id: UtteranceId) -> Result<Transcript, SttError> {
        let (utt, cfg) = {
            let mut st = self.lock();
            let utt = st
                .utterances
                .remove(&id)
                .ok_or(SttError::UnknownUtterance(id))?;
            (utt, st.cfg.clone())
        };
        let started = Instant::now();
        let pcm = samples_of(&utt.frames);
        let stats: AudioStats = guard::audio_stats(&pcm);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let audio_ms = (stats.seconds * 1000.0) as u32;
        let elapsed = |t: Instant| u32::try_from(t.elapsed().as_millis()).unwrap_or(u32::MAX);
        if guard::reject_reason(&stats).is_some() {
            if utt.on_primary {
                self.primary.cancel(id).await;
            } else {
                self.fallback.cancel(id).await;
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let speech_ms = stats.speech_ms as u32;
            self.lock().events.push(SttEvent::GateRejected {
                utterance: id,
                speech_ms,
            });
            self.record(Sample {
                engine: "brak",
                audio_ms,
                processing_ms: 0,
                flags: Vec::new(),
                fallback_used: false,
                skipped: true,
            });
            return Ok(Transcript::empty_final(id));
        }
        if !utt.on_primary {
            let mut t = self.fallback.end_utterance(id).await?;
            if guard::is_hallucination(&t.text) {
                t.text.clear();
                t.words.clear();
            }
            self.record(Sample {
                engine: "zapasowy",
                audio_ms,
                processing_ms: elapsed(started),
                flags: Vec::new(),
                fallback_used: true,
                skipped: false,
            });
            return Ok(t);
        }
        let primary = self.primary.end_utterance(id).await;
        let now = self.now();
        let (mut result, flags, fallback_used) = match primary {
            Ok(t) => {
                self.lock().breaker.record(true, now);
                let detected = (!t.lang.is_empty()).then_some(t.lang.as_str());
                let flags =
                    guard::quality_flags(&t.text, &stats, requested_lang(&cfg), detected, false);
                if flags.is_empty() {
                    (t, flags, false)
                } else {
                    // Podejrzany wynik: to samo nagranie rozpoznaje silnik zapasowy; jego niepusty
                    // tekst wygrywa. Tekst w nieoczekiwanym języku bez zastępstwa jest kasowany.
                    match self.replay(id, &utt.frames).await {
                        Ok(second) if !second.text.trim().is_empty() => (second, flags, true),
                        _ => {
                            let mut t = t;
                            if flags.contains(&Flag::UnexpectedLanguage) {
                                t.text.clear();
                                t.words.clear();
                            }
                            (t, flags, false)
                        }
                    }
                }
            }
            Err(_) => {
                self.lock().breaker.record(false, now);
                (self.replay(id, &utt.frames).await?, Vec::new(), true)
            }
        };
        if guard::is_hallucination(&result.text) {
            result.text.clear();
            result.words.clear();
        }
        self.record(Sample {
            engine: if fallback_used { "zapasowy" } else { "glowny" },
            audio_ms,
            processing_ms: elapsed(started),
            flags,
            fallback_used,
            skipped: false,
        });
        Ok(result)
    }

    async fn cancel(&self, id: UtteranceId) {
        let on_primary = self.lock().utterances.remove(&id).map(|u| u.on_primary);
        match on_primary {
            Some(true) => self.primary.cancel(id).await,
            Some(false) => self.fallback.cancel(id).await,
            None => {}
        }
    }

    fn health(&self) -> Health {
        let open = self.lock().breaker.snapshot().0 != BreakerState::Closed;
        if open {
            return Health::Degraded("silnik główny odstawiony — rozpoznaje zapasowy".into());
        }
        match self.primary.health() {
            Health::Failed(_) => self.fallback.health(),
            other => other,
        }
    }

    fn take_events(&self) -> Vec<SttEvent> {
        let mut events = std::mem::take(&mut self.lock().events);
        events.extend(self.primary.take_events());
        events.extend(self.fallback.take_events());
        events
    }
}
