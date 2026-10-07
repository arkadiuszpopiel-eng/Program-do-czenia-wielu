//! [`MixerOutput`] — wspólna implementacja [`OutputStream`] na mikserze (używają jej `-impl` i `-fake`):
//! downmix, resampling do częstotliwości urządzenia, normalizacja głośności per źródło, kolejka RT.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use crate::OutputStream;
use crate::events::AudioEvent;
use crate::frame::{AudioFormat, Frame};
use crate::gain::LoudnessNormalizer;
use crate::mixer::{MixerControl, MixerReport};
use crate::resample::Resampler;
use crate::types::{AudioError, Ducking, Lane, LoopLatency, PlaybackPosition, SourceId};

/// Czas wygaszenia przy `stop_all` (SPEC: cisza ≤ 20 ms).
pub const STOP_FADE: Duration = Duration::from_millis(5);

/// Stan urządzenia widziany przez stronę sterującą (aktualizuje wątek urządzenia).
#[derive(Debug, Default)]
pub struct DeviceStatus {
    /// Opóźnienie wyjścia (ns): bufor urządzenia + sprzęt.
    pub output_latency_ns: AtomicU64,
    /// Strumień zamknięty (np. urządzenie odłączone, wątek zakończony).
    pub closed: AtomicBool,
}

impl DeviceStatus {
    /// Ustawia opóźnienie wyjścia.
    pub fn set_output_latency(&self, d: Duration) {
        self.output_latency_ns.store(
            u64::try_from(d.as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    }
}

/// Wyjście audio na mikserze.
#[derive(Debug)]
pub struct MixerOutput {
    control: MixerControl,
    format: AudioFormat,
    status: Arc<DeviceStatus>,
    input_latency: Duration,
    calibrated: Option<Duration>,
    normalize: bool,
    resamplers: HashMap<u64, Resampler>,
    normalizers: HashMap<SourceId, LoudnessNormalizer>,
    sources: BTreeMap<u64, SourceId>,
}

impl MixerOutput {
    /// Nowe wyjście; `format` = format urządzenia, `status` współdzielony z wątkiem urządzenia.
    pub fn new(control: MixerControl, format: AudioFormat, status: Arc<DeviceStatus>) -> Self {
        Self {
            control,
            format,
            status,
            input_latency: Duration::ZERO,
            calibrated: None,
            normalize: true,
            resamplers: HashMap::new(),
            normalizers: HashMap::new(),
            sources: BTreeMap::new(),
        }
    }

    /// Włącza/wyłącza normalizację głośności (`[voice.audio] normalize`).
    pub fn set_normalize(&mut self, on: bool) {
        self.normalize = on;
    }

    /// Opóźnienie wejścia (do `LoopLatency`).
    pub fn set_input_latency(&mut self, d: Duration) {
        self.input_latency = d;
    }

    /// Dostęp do miksera (diagnostyka, testy).
    pub fn control(&mut self) -> &mut MixerControl {
        &mut self.control
    }

    fn closed(&self) -> Result<(), AudioError> {
        if self.status.closed.load(Ordering::Relaxed) {
            Err(AudioError::Closed)
        } else {
            Ok(())
        }
    }
}

impl OutputStream for MixerOutput {
    fn format(&self) -> AudioFormat {
        self.format
    }

    fn play(&mut self, source: &SourceId, utterance: u64, chunk: &Frame) -> Result<(), AudioError> {
        self.closed()?;
        chunk.format.validate()?;
        let mono = chunk.to_mono();
        let rate = self.format.sample_rate;
        let resampler = self
            .resamplers
            .entry(utterance)
            .or_insert_with(|| Resampler::new(chunk.format.sample_rate, rate));
        if resampler.from_rate() != chunk.format.sample_rate {
            *resampler = Resampler::new(chunk.format.sample_rate, rate);
        }
        let mut samples = Vec::with_capacity(mono.len() * 2 + 64);
        resampler.process(&mono, &mut samples);
        if self.normalize && source.lane() == Lane::Voice {
            self.normalizers
                .entry(source.clone())
                .or_default()
                .process(&mut samples);
        }
        self.sources
            .entry(utterance)
            .or_insert_with(|| source.clone());
        self.control.enqueue(source.lane(), utterance, &samples)
    }

    fn end_utterance(&mut self, utterance: u64) -> Result<(), AudioError> {
        let Some(source) = self.sources.get(&utterance).cloned() else {
            return Ok(());
        };
        if let Some(mut r) = self.resamplers.remove(&utterance) {
            let mut tail = Vec::new();
            r.flush(&mut tail);
            if self.normalize && source.lane() == Lane::Voice {
                self.normalizers
                    .entry(source.clone())
                    .or_default()
                    .process(&mut tail);
            }
            self.control.enqueue(source.lane(), utterance, &tail)?;
        }
        self.control.end(source.lane(), utterance)
    }

    fn duck(&mut self, ducking: Ducking) -> Result<(), AudioError> {
        self.control.duck(ducking)
    }

    fn unduck(&mut self, release: Duration) -> Result<(), AudioError> {
        self.control.unduck(release)
    }

    fn stop_all(&mut self) -> Result<(), AudioError> {
        self.control.stop(Lane::Voice, STOP_FADE)?;
        self.control.stop(Lane::Effects, STOP_FADE)?;
        self.resamplers.clear();
        Ok(())
    }

    fn position(&mut self, utterance: u64) -> Option<PlaybackPosition> {
        let (rendered, queued, state) = self.control.position(utterance)?;
        Some(PlaybackPosition {
            utterance,
            rendered_samples: rendered,
            queued_samples: queued,
            sample_rate: self.format.sample_rate,
            output_latency: self.latency().output,
            state,
        })
    }

    fn poll_events(&mut self) -> Vec<AudioEvent> {
        let reports = self.control.poll();
        let mut out = Vec::with_capacity(reports.len());
        for r in reports {
            match r {
                MixerReport::Started { utterance, .. } => out.push(AudioEvent::PlaybackStarted {
                    utterance,
                    source: self.sources.get(&utterance).cloned(),
                }),
                MixerReport::Finished {
                    utterance,
                    rendered,
                    stopped,
                    ..
                } => out.push(AudioEvent::PlaybackFinished {
                    utterance,
                    rendered_samples: rendered,
                    stopped,
                }),
                MixerReport::Underrun { lane } => out.push(AudioEvent::Underrun { lane }),
            }
        }
        if self.sources.len() > 512
            && let Some(first) = self.sources.keys().next().copied()
        {
            self.sources.remove(&first);
        }
        out
    }

    fn drain_reference(&mut self) -> Vec<Frame> {
        self.control.drain_reference()
    }

    fn latency(&self) -> LoopLatency {
        LoopLatency {
            output: Duration::from_nanos(self.status.output_latency_ns.load(Ordering::Relaxed)),
            input: self.input_latency,
            calibrated_loop: self.calibrated,
        }
    }

    fn set_calibrated_loop(&mut self, loop_latency: Duration) {
        self.calibrated = Some(loop_latency);
    }

    fn duck_gain(&self) -> f32 {
        self.control.duck_gain()
    }
}
