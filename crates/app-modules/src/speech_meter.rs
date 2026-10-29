//! Poziom mowy agentki dla interfejsu: awatar rusza ustami w rytm tego, co gra głośnik.
//! Fragment syntezy trafia do miksera cały i gra się potem w czasie rzeczywistym, więc poziom
//! liczymy z samego fragmentu (obwiednia co [`STEP`]) i wysyłamy w chwilach, w których dana część
//! zabrzmi: fragmenty ustawiają się jeden za drugim, tak jak w kolejce miksera. Stop albo nowa
//! wypowiedź unieważnia zaplanowane zdarzenia. Znacznik ([`SpeechMeter::mark_next`], np. czytane
//! zdanie) wychodzi razem z pierwszym poziomem następnego fragmentu — wyróżnienie nie wyprzedza głosu.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::time::Instant;
use voice_audio_contract::{
    AudioError, AudioEvent, AudioFormat, Ducking, Frame, LoopLatency, OutputStream,
    PlaybackPosition, SourceId,
};

use app_api::dto::AlfaEvent;
use app_api::events::EventHub;

/// Odstęp między pomiarami (jak poziom mikrofonu: ok. 30 na sekundę).
pub const STEP: Duration = Duration::from_millis(34);

/// Obwiednia fragmentu: poziom 0..1 (RMS w skali −50…0 dBFS) dla kolejnych okien [`STEP`].
pub fn envelope(pcm: &[f32], format: AudioFormat) -> Vec<f64> {
    let per_second = u64::from(format.sample_rate.max(1)) * u64::from(format.channels.max(1));
    let window = usize::try_from(per_second * STEP.as_millis() as u64 / 1000)
        .unwrap_or(usize::MAX)
        .max(1);
    pcm.chunks(window)
        .map(|part| {
            let energy: f64 = part.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
            let rms = (energy / part.len() as f64).sqrt();
            if rms <= 1e-5 {
                0.0
            } else {
                ((20.0 * rms.log10() + 50.0) / 50.0).clamp(0.0, 1.0)
            }
        })
        .collect()
}

/// Odbiorca zdarzeń miernika.
pub type Sink = Arc<dyn Fn(AlfaEvent) + Send + Sync>;

#[derive(Debug)]
struct Timeline {
    /// Koniec ostatnio zaplanowanego fragmentu — kolejny zaczyna się nie wcześniej.
    busy_until: Instant,
}

/// Wysyła `SpeechLevel` do interfejsu w rytm odtwarzania.
#[derive(Clone)]
pub struct SpeechMeter {
    events: Sink,
    /// Numer „pokolenia”: stop zwiększa go i zaplanowane zadania milkną.
    generation: Arc<AtomicU64>,
    timeline: Arc<Mutex<Timeline>>,
    /// Zdarzenie do wysłania, gdy zabrzmi następny fragment.
    marker: Arc<Mutex<Option<AlfaEvent>>>,
}

impl SpeechMeter {
    /// Miernik wysyłający zdarzenia do interfejsu.
    pub fn new(events: EventHub) -> Self {
        Self::with_sink(Arc::new(move |event| events.emit(event)))
    }

    /// Miernik z własnym odbiorcą zdarzeń (testy).
    pub fn with_sink(events: Sink) -> Self {
        Self {
            events,
            generation: Arc::default(),
            timeline: Arc::new(Mutex::new(Timeline {
                busy_until: Instant::now(),
            })),
            marker: Arc::default(),
        }
    }

    /// Zdarzenie `event` wyjdzie w chwili, gdy zacznie brzmieć następny fragment.
    pub fn mark_next(&self, event: AlfaEvent) {
        *self.marker.lock().unwrap_or_else(PoisonError::into_inner) = Some(event);
    }

    /// Planuje poziomy dla fragmentu mowy agentki `agent`, który właśnie trafił do miksera.
    pub fn schedule(&self, agent: &str, chunk: &Frame) {
        let levels = envelope(&chunk.pcm, chunk.format);
        let marker = self
            .marker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if levels.is_empty() {
            // Pusty fragment nic nie gra — znacznik czeka na następny.
            if let Some(event) = marker {
                self.mark_next(event);
            }
            return;
        }
        let length = STEP * u32::try_from(levels.len()).unwrap_or(u32::MAX);
        let start = {
            let mut timeline = self.timeline.lock().unwrap_or_else(PoisonError::into_inner);
            let start = timeline.busy_until.max(Instant::now());
            timeline.busy_until = start + length;
            start
        };
        let (events, generation) = (self.events.clone(), self.generation.clone());
        let timeline = self.timeline.clone();
        let (mine, agent) = (generation.load(Ordering::SeqCst), agent.to_owned());
        tokio::spawn(async move {
            if let Some(event) = marker {
                tokio::time::sleep_until(start).await;
                if generation.load(Ordering::SeqCst) != mine {
                    return;
                }
                events(event);
            }
            for (index, level) in levels.into_iter().enumerate() {
                tokio::time::sleep_until(start + STEP * u32::try_from(index).unwrap_or(u32::MAX))
                    .await;
                if generation.load(Ordering::SeqCst) != mine {
                    return;
                }
                events(AlfaEvent::SpeechLevel {
                    agent: agent.clone(),
                    level,
                });
            }
            tokio::time::sleep_until(start + length).await;
            // Cisza na końcu tylko wtedy, gdy za tym fragmentem nie czeka następny.
            let last = timeline
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .busy_until
                <= start + length;
            if last && generation.load(Ordering::SeqCst) == mine {
                events(AlfaEvent::SpeechLevel { agent, level: 0.0 });
            }
        });
    }

    /// Mowa przerwana: zaplanowane poziomy przepadają, interfejs dostaje ciszę.
    pub fn silence(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.marker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        self.timeline
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .busy_until = Instant::now();
        (self.events)(AlfaEvent::SpeechLevel {
            agent: String::new(),
            level: 0.0,
        });
    }
}

/// Wyjście audio, które każdy fragment mowy agentki zgłasza miernikowi. Reszta bez zmian.
pub struct MeteredOutput {
    inner: Box<dyn OutputStream>,
    meter: SpeechMeter,
}

impl MeteredOutput {
    /// Opakowuje wyjście `inner`.
    pub fn new(inner: Box<dyn OutputStream>, meter: SpeechMeter) -> Self {
        Self { inner, meter }
    }
}

impl OutputStream for MeteredOutput {
    fn format(&self) -> AudioFormat {
        self.inner.format()
    }

    fn play(&mut self, source: &SourceId, utterance: u64, chunk: &Frame) -> Result<(), AudioError> {
        self.inner.play(source, utterance, chunk)?;
        if let SourceId::Tts(persona) | SourceId::Filler(persona) = source {
            self.meter.schedule(&persona.to_string(), chunk);
        }
        Ok(())
    }

    fn end_utterance(&mut self, utterance: u64) -> Result<(), AudioError> {
        self.inner.end_utterance(utterance)
    }

    fn duck(&mut self, ducking: Ducking) -> Result<(), AudioError> {
        self.inner.duck(ducking)
    }

    fn unduck(&mut self, release: Duration) -> Result<(), AudioError> {
        self.inner.unduck(release)
    }

    fn stop_all(&mut self) -> Result<(), AudioError> {
        self.meter.silence();
        self.inner.stop_all()
    }

    fn position(&mut self, utterance: u64) -> Option<PlaybackPosition> {
        self.inner.position(utterance)
    }

    fn poll_events(&mut self) -> Vec<AudioEvent> {
        self.inner.poll_events()
    }

    fn drain_reference(&mut self) -> Vec<Frame> {
        self.inner.drain_reference()
    }

    fn latency(&self) -> LoopLatency {
        self.inner.latency()
    }

    fn set_calibrated_loop(&mut self, loop_latency: Duration) {
        self.inner.set_calibrated_loop(loop_latency);
    }

    fn duck_gain(&self) -> f32 {
        self.inner.duck_gain()
    }
}

/// Zdarzenie czytanego zdania `index` z `sentences` (poza zakresem — koniec czytania, pusty tekst).
pub fn sentence_segment(turn_id: &str, sentences: &[String], index: u32) -> AlfaEvent {
    AlfaEvent::SpeechSegment {
        turn_id: turn_id.to_owned(),
        index,
        of: u32::try_from(sentences.len()).unwrap_or(u32::MAX),
        text: usize::try_from(index)
            .ok()
            .and_then(|i| sentences.get(i))
            .cloned()
            .unwrap_or_default(),
    }
}
