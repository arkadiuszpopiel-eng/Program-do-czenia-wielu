//! Strona sterująca miksera (poza wątkiem RT).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use rtrb::{Consumer, Producer};

use super::render::MAX_SEGMENTS;
use super::{Cmd, MixerConfig, RefBlock, Report, Shared};
use crate::frame::{Frame, MediaTime};
use crate::gain::db_to_gain;
use crate::types::{AudioError, Ducking, Lane, PlaybackState};

/// Ile zakończonych wypowiedzi pamiętać (pozycje do „usłyszanego prefiksu”).
const KEEP_FINISHED: usize = 256;

/// Zdarzenie odtwarzania odczytane z wątku RT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixerReport {
    /// Wypowiedź zaczęła grać.
    Started {
        /// Tor.
        lane: Lane,
        /// Wypowiedź.
        utterance: u64,
    },
    /// Wypowiedź skończyła grać (albo została przerwana).
    Finished {
        /// Tor.
        lane: Lane,
        /// Wypowiedź.
        utterance: u64,
        /// Wyrenderowane próbki.
        rendered: u64,
        /// Przerwana przez `stop`.
        stopped: bool,
    },
    /// Tor głosu głodny (TTS nie nadąża) — `voice.audio.underrun`.
    Underrun {
        /// Tor.
        lane: Lane,
    },
}

/// Liczniki miksera.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MixerStats {
    /// Wszystkie wyrenderowane próbki (na kanał).
    pub rendered: u64,
    /// Epizody głodu toru.
    pub underruns: u64,
    /// Raporty zgubione (pełna kolejka raportów).
    pub dropped_reports: u64,
    /// Próbki referencji AEC zgubione (konsument nie nadąża).
    pub dropped_reference: u64,
}

#[derive(Debug, Clone, Copy)]
struct UttInfo {
    lane: Lane,
    queued: u64,
    rendered: u64,
    state: PlaybackState,
    ended: bool,
}

/// Strona sterująca miksera: kolejkuje próbki (już w częstotliwości urządzenia, mono), ducking,
/// zatrzymanie, pozycje wypowiedzi, odczyt referencji dla AEC.
#[derive(Debug)]
pub struct MixerControl {
    config: MixerConfig,
    cmds: Producer<Cmd>,
    reports: Consumer<Report>,
    voice: Producer<f32>,
    effects: Producer<f32>,
    reference: Option<(Consumer<f32>, Consumer<RefBlock>)>,
    shared: Arc<Shared>,
    utterances: BTreeMap<u64, UttInfo>,
    pending_segments: [usize; 2],
    backlog: Vec<MixerReport>,
}

fn lane_idx(lane: Lane) -> usize {
    match lane {
        Lane::Voice => 0,
        Lane::Effects => 1,
    }
}

impl MixerControl {
    pub(crate) fn new(
        config: MixerConfig,
        cmds: Producer<Cmd>,
        reports: Consumer<Report>,
        voice: Producer<f32>,
        effects: Producer<f32>,
        reference: Option<(Consumer<f32>, Consumer<RefBlock>)>,
        shared: Arc<Shared>,
    ) -> Self {
        Self {
            config,
            cmds,
            reports,
            voice,
            effects,
            reference,
            shared,
            utterances: BTreeMap::new(),
            pending_segments: [0, 0],
            backlog: Vec::new(),
        }
    }

    /// Częstotliwość urządzenia.
    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate
    }

    /// Wypowiedź toru głosu otwarta (niezamknięta `end`), inna niż `utterance`.
    pub fn open_voice_utterance(&self, except: u64) -> Option<u64> {
        self.utterances.iter().find_map(|(id, u)| {
            (u.lane == Lane::Voice
                && *id != except
                && !u.ended
                && matches!(u.state, PlaybackState::Queued | PlaybackState::Playing))
            .then_some(*id)
        })
    }

    /// Kolejkuje próbki mono (częstotliwość urządzenia). Tor głosu przyjmuje nową wypowiedź dopiero,
    /// gdy poprzednia została zamknięta (`end`) — mikser nie miesza dwóch TTS.
    pub fn enqueue(
        &mut self,
        lane: Lane,
        utterance: u64,
        samples: &[f32],
    ) -> Result<(), AudioError> {
        if let Some(info) = self.utterances.get(&utterance) {
            if info.state == PlaybackState::Stopped || info.ended {
                return Ok(()); // spóźnione fragmenty przerwanej / zamkniętej wypowiedzi
            }
        } else if lane == Lane::Voice
            && let Some(playing) = self.open_voice_utterance(utterance)
        {
            return Err(AudioError::VoiceBusy { playing });
        }
        if samples.is_empty() {
            return Ok(());
        }
        let len = u32::try_from(samples.len()).map_err(|_| AudioError::QueueFull)?;
        self.pump();
        let idx = lane_idx(lane);
        if self.cmds.slots() < 1 || self.pending_segments[idx] >= MAX_SEGMENTS - 1 {
            return Err(AudioError::QueueFull);
        }
        let ring = match lane {
            Lane::Voice => &mut self.voice,
            Lane::Effects => &mut self.effects,
        };
        ring.push_entire_slice(samples)
            .map_err(|_| AudioError::QueueFull)?;
        self.cmds
            .push(Cmd::Chunk {
                lane,
                utterance,
                len,
            })
            .map_err(|_| AudioError::QueueFull)?;
        self.pending_segments[idx] += 1;
        let info = self.utterances.entry(utterance).or_insert(UttInfo {
            lane,
            queued: 0,
            rendered: 0,
            state: PlaybackState::Queued,
            ended: false,
        });
        info.queued += u64::from(len);
        Ok(())
    }

    /// Zamyka wypowiedź (po ostatnim fragmencie): raport `Finished` po wybrzmieniu.
    pub fn end(&mut self, lane: Lane, utterance: u64) -> Result<(), AudioError> {
        let Some(info) = self.utterances.get_mut(&utterance) else {
            return Ok(());
        };
        if info.ended || info.state == PlaybackState::Stopped {
            return Ok(());
        }
        info.ended = true;
        self.cmds
            .push(Cmd::End { lane, utterance })
            .map_err(|_| AudioError::QueueFull)?;
        self.pending_segments[lane_idx(lane)] += 1;
        Ok(())
    }

    /// Ducking toru głosu (rampa liniowa).
    pub fn duck(&mut self, ducking: Ducking) -> Result<(), AudioError> {
        ducking.validate()?;
        self.ramp_to(db_to_gain(ducking.gain_db), ducking.attack)
    }

    /// Powrót do pełnej głośności głosu.
    pub fn unduck(&mut self, release: Duration) -> Result<(), AudioError> {
        self.ramp_to(1.0, release)
    }

    fn ramp_to(&mut self, target: f32, ramp: Duration) -> Result<(), AudioError> {
        let ramp = MediaTime(u64::try_from(ramp.as_nanos()).unwrap_or(u64::MAX))
            .to_samples(self.config.sample_rate);
        self.cmds
            .push(Cmd::Duck {
                target,
                ramp: u32::try_from(ramp).unwrap_or(u32::MAX),
            })
            .map_err(|_| AudioError::QueueFull)
    }

    /// Bieżące wzmocnienie duckingu (liniowe) po ostatnim renderze.
    pub fn duck_gain(&self) -> f32 {
        f32::from_bits(self.shared.duck_gain_bits.load(Ordering::Relaxed))
    }

    /// Zatrzymuje tor: wygaszenie `fade`, potem wyrzucenie kolejki. Wypowiedzi → `Stopped`.
    pub fn stop(&mut self, lane: Lane, fade: Duration) -> Result<(), AudioError> {
        let fade = MediaTime(u64::try_from(fade.as_nanos()).unwrap_or(u64::MAX))
            .to_samples(self.config.sample_rate);
        self.cmds
            .push(Cmd::Stop {
                lane,
                fade: u32::try_from(fade).unwrap_or(u32::MAX),
            })
            .map_err(|_| AudioError::QueueFull)?;
        for info in self.utterances.values_mut() {
            if info.lane == lane
                && matches!(info.state, PlaybackState::Queued | PlaybackState::Playing)
            {
                info.state = PlaybackState::Stopped;
                info.ended = true;
            }
        }
        Ok(())
    }

    /// Głośność toru (dB).
    pub fn set_gain(&mut self, lane: Lane, gain_db: f32) -> Result<(), AudioError> {
        self.cmds
            .push(Cmd::Gain {
                lane,
                gain: db_to_gain(gain_db),
            })
            .map_err(|_| AudioError::QueueFull)
    }

    /// Odbiera raporty z wątku RT (także te zebrane przy `enqueue`/`position`) i aktualizuje stan.
    pub fn poll(&mut self) -> Vec<MixerReport> {
        self.pump();
        std::mem::take(&mut self.backlog)
    }

    fn pump(&mut self) {
        let out = &mut self.backlog;
        while let Ok(report) = self.reports.pop() {
            match report {
                Report::Started { lane, utterance } => {
                    if let Some(u) = self.utterances.get_mut(&utterance)
                        && u.state == PlaybackState::Queued
                    {
                        u.state = PlaybackState::Playing;
                    }
                    out.push(MixerReport::Started { lane, utterance });
                }
                Report::Finished {
                    lane,
                    utterance,
                    rendered,
                    stopped,
                } => {
                    if let Some(u) = self.utterances.get_mut(&utterance) {
                        if u.state != PlaybackState::Finished {
                            u.rendered = u.rendered.max(rendered);
                        }
                        if !stopped && u.state != PlaybackState::Stopped {
                            u.state = PlaybackState::Finished;
                        } else {
                            u.state = PlaybackState::Stopped;
                        }
                    }
                    out.push(MixerReport::Finished {
                        lane,
                        utterance,
                        rendered,
                        stopped,
                    });
                }
                Report::Underrun { lane } => out.push(MixerReport::Underrun { lane }),
                Report::Overflow { .. } => {}
            }
        }
        self.pending_segments = [0, 0];
        self.prune();
    }

    fn prune(&mut self) {
        let done: Vec<u64> = self
            .utterances
            .iter()
            .filter(|(_, u)| matches!(u.state, PlaybackState::Finished | PlaybackState::Stopped))
            .map(|(id, _)| *id)
            .collect();
        if done.len() > KEEP_FINISHED {
            for id in &done[..done.len() - KEEP_FINISHED] {
                self.utterances.remove(id);
            }
        }
    }

    /// Pozycja wypowiedzi: (wyrenderowane, zakolejkowane, stan).
    pub fn position(&mut self, utterance: u64) -> Option<(u64, u64, PlaybackState)> {
        self.pump();
        let info = *self.utterances.get(&utterance)?;
        let rendered = match (info.state, self.shared.progress.read()) {
            (PlaybackState::Playing, Some((u, r))) if u == utterance => r,
            _ => info.rendered,
        };
        Some((rendered.min(info.queued), info.queued, info.state))
    }

    /// Referencja AEC: to, co zagrało (mono, częstotliwość urządzenia, czas odtworzenia).
    pub fn drain_reference(&mut self) -> Vec<Frame> {
        let rate = self.config.sample_rate;
        let Some((samples, blocks)) = self.reference.as_mut() else {
            return Vec::new();
        };
        let mut frames = Vec::new();
        while let Ok(block) = blocks.pop() {
            let n = (block.len as usize).min(samples.slots());
            let mut pcm = vec![0.0f32; n];
            if samples.pop_entire_slice(&mut pcm).is_err() {
                break;
            }
            frames.push(Frame::mono(pcm, rate, block.ts));
        }
        frames
    }

    /// Liczniki.
    pub fn stats(&self) -> MixerStats {
        MixerStats {
            rendered: self.shared.rendered.load(Ordering::Relaxed),
            underruns: self.shared.underruns.load(Ordering::Relaxed),
            dropped_reports: self.shared.dropped_reports.load(Ordering::Relaxed),
            dropped_reference: self.shared.dropped_reference.load(Ordering::Relaxed),
        }
    }
}
