//! Strona RT miksera: wywoływana z wątku urządzenia. Zero alokacji, zero blokad, zero logowania —
//! wyłącznie kolejki SPSC (`rtrb`) o stałej pojemności i atomiki.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use rtrb::{Consumer, Producer};

use super::{Cmd, RefBlock, Report, Shared};
use crate::frame::MediaTime;
use crate::types::Lane;

/// Maksymalna liczba segmentów (fragmentów) oczekujących w torze.
pub(crate) const MAX_SEGMENTS: usize = 4096;

#[derive(Debug, Clone, Copy)]
struct Segment {
    utterance: u64,
    remaining: u32,
    end: bool,
}

#[derive(Debug)]
struct LaneState {
    lane: Lane,
    samples: Consumer<f32>,
    segments: VecDeque<Segment>,
    current: Option<u64>,
    rendered: u64,
    gain: f32,
    starving: bool,
    /// Trwa zatrzymanie (wygaszanie, potem wyrzucenie materiału sprzed `Stop`).
    stopping: bool,
    /// Segmenty sprzed polecenia `Stop`, które należy wygasić i wyrzucić.
    stop_pending: usize,
    fade_left: u32,
    fade_total: u32,
}

impl LaneState {
    fn new(lane: Lane, samples: Consumer<f32>) -> Self {
        Self {
            lane,
            samples,
            segments: VecDeque::with_capacity(MAX_SEGMENTS),
            current: None,
            rendered: 0,
            gain: 1.0,
            starving: false,
            stopping: false,
            stop_pending: 0,
            fade_left: 0,
            fade_total: 0,
        }
    }

    fn push_segment(&mut self, seg: Segment, reports: &mut Producer<Report>, shared: &Shared) {
        if self.segments.len() < MAX_SEGMENTS {
            self.segments.push_back(seg);
        } else {
            // Nie powinno się zdarzyć (sterowanie pilnuje limitu) — próbki trzeba wyrzucić,
            // by kolejka próbek nie rozjechała się z segmentami.
            skip_samples(&mut self.samples, seg.remaining as usize);
            send(reports, shared, Report::Overflow { lane: self.lane });
        }
    }

    fn begin_stop(&mut self, fade: u32) {
        self.stopping = true;
        self.stop_pending = self.segments.len();
        self.fade_total = fade.max(1);
        self.fade_left = fade;
    }

    /// Wyrzuca segmenty sprzed `Stop` razem z ich próbkami i zgłasza przerwane wypowiedzi.
    fn flush_stopped(&mut self, reports: &mut Producer<Report>, shared: &Shared) {
        if let Some(u) = self.current.take() {
            send(
                reports,
                shared,
                Report::Finished {
                    lane: self.lane,
                    utterance: u,
                    rendered: self.rendered,
                    stopped: true,
                },
            );
        }
        let mut last: Option<u64> = None;
        for _ in 0..self.stop_pending {
            let Some(seg) = self.segments.pop_front() else {
                break;
            };
            skip_samples(&mut self.samples, seg.remaining as usize);
            if last != Some(seg.utterance) {
                last = Some(seg.utterance);
                send(
                    reports,
                    shared,
                    Report::Finished {
                        lane: self.lane,
                        utterance: seg.utterance,
                        rendered: 0,
                        stopped: true,
                    },
                );
            }
        }
        self.stopping = false;
        self.stop_pending = 0;
        self.fade_left = 0;
        self.rendered = 0;
        self.starving = false;
    }

    fn next_sample(&mut self, reports: &mut Producer<Report>, shared: &Shared) -> f32 {
        if self.stopping && (self.fade_left == 0 || self.stop_pending == 0) {
            self.flush_stopped(reports, shared);
        }
        loop {
            let Some(seg) = self.segments.front_mut() else {
                if self.current.is_some() && !self.starving {
                    self.starving = true;
                    shared.underruns.fetch_add(1, Ordering::Relaxed);
                    send(reports, shared, Report::Underrun { lane: self.lane });
                }
                return 0.0;
            };
            if seg.remaining > 0 {
                let utterance = seg.utterance;
                seg.remaining -= 1;
                if self.current != Some(utterance) {
                    if let Some(prev) = self.current {
                        send(
                            reports,
                            shared,
                            Report::Finished {
                                lane: self.lane,
                                utterance: prev,
                                rendered: self.rendered,
                                stopped: false,
                            },
                        );
                    }
                    self.current = Some(utterance);
                    self.rendered = 0;
                    send(
                        reports,
                        shared,
                        Report::Started {
                            lane: self.lane,
                            utterance,
                        },
                    );
                }
                self.starving = false;
                self.rendered += 1;
                let mut s = self.samples.pop().unwrap_or(0.0) * self.gain;
                if self.stopping {
                    s *= self.fade_left as f32 / self.fade_total as f32;
                    self.fade_left = self.fade_left.saturating_sub(1);
                }
                return s;
            }
            self.pop_exhausted(reports, shared);
        }
    }

    /// Zdejmuje z czoła wyczerpany segment (albo znacznik końca) i zgłasza koniec wypowiedzi.
    fn pop_exhausted(&mut self, reports: &mut Producer<Report>, shared: &Shared) {
        let Some(seg) = self.segments.pop_front() else {
            return;
        };
        if self.stopping {
            self.stop_pending = self.stop_pending.saturating_sub(1);
            if self.stop_pending == 0 {
                // Materiał sprzed `Stop` skończył się przed końcem wygaszania.
                self.flush_stopped(reports, shared);
                return;
            }
        }
        if seg.end && (self.current == Some(seg.utterance) || self.current.is_none()) {
            let rendered = if self.current.is_some() {
                self.rendered
            } else {
                0
            };
            send(
                reports,
                shared,
                Report::Finished {
                    lane: self.lane,
                    utterance: seg.utterance,
                    rendered,
                    stopped: false,
                },
            );
            self.current = None;
            self.rendered = 0;
        }
    }

    /// Po renderze: domyka wypowiedzi, których ostatnia próbka właśnie zagrała.
    fn settle(&mut self, reports: &mut Producer<Report>, shared: &Shared) {
        while self.segments.front().is_some_and(|s| s.remaining == 0) {
            self.pop_exhausted(reports, shared);
        }
    }
}

fn skip_samples(samples: &mut Consumer<f32>, n: usize) {
    let n = n.min(samples.slots());
    if let Ok(chunk) = samples.read_chunk(n) {
        chunk.commit_all();
    }
}

fn send(reports: &mut Producer<Report>, shared: &Shared, report: Report) {
    if reports.push(report).is_err() {
        shared.dropped_reports.fetch_add(1, Ordering::Relaxed);
    }
}

/// Strona RT miksera (przekazywana do wątku urządzenia).
#[derive(Debug)]
pub struct MixerRender {
    cmds: Consumer<Cmd>,
    reports: Producer<Report>,
    voice: LaneState,
    effects: LaneState,
    reference: Option<(Producer<f32>, Producer<RefBlock>)>,
    shared: Arc<Shared>,
    duck_gain: f32,
    duck_step: f32,
    duck_left: u32,
}

impl MixerRender {
    pub(crate) fn new(
        cmds: Consumer<Cmd>,
        reports: Producer<Report>,
        voice: Consumer<f32>,
        effects: Consumer<f32>,
        reference: Option<(Producer<f32>, Producer<RefBlock>)>,
        shared: Arc<Shared>,
    ) -> Self {
        Self {
            cmds,
            reports,
            voice: LaneState::new(Lane::Voice, voice),
            effects: LaneState::new(Lane::Effects, effects),
            reference,
            shared,
            duck_gain: 1.0,
            duck_step: 0.0,
            duck_left: 0,
        }
    }

    fn lane_mut(&mut self, lane: Lane) -> &mut LaneState {
        match lane {
            Lane::Voice => &mut self.voice,
            Lane::Effects => &mut self.effects,
        }
    }

    fn apply_commands(&mut self) {
        while let Ok(cmd) = self.cmds.pop() {
            match cmd {
                Cmd::Chunk {
                    lane,
                    utterance,
                    len,
                } => {
                    let seg = Segment {
                        utterance,
                        remaining: len,
                        end: false,
                    };
                    let (st, reports, shared) = self.split(lane);
                    st.push_segment(seg, reports, shared);
                }
                Cmd::End { lane, utterance } => {
                    let seg = Segment {
                        utterance,
                        remaining: 0,
                        end: true,
                    };
                    let (st, reports, shared) = self.split(lane);
                    st.push_segment(seg, reports, shared);
                }
                Cmd::Duck { target, ramp } => {
                    let ramp = ramp.max(1);
                    self.duck_step = (target - self.duck_gain) / ramp as f32;
                    self.duck_left = ramp;
                }
                Cmd::Stop { lane, fade } => self.lane_mut(lane).begin_stop(fade),
                Cmd::Gain { lane, gain } => self.lane_mut(lane).gain = gain,
            }
        }
    }

    fn split(&mut self, lane: Lane) -> (&mut LaneState, &mut Producer<Report>, &Shared) {
        let st = match lane {
            Lane::Voice => &mut self.voice,
            Lane::Effects => &mut self.effects,
        };
        (st, &mut self.reports, &self.shared)
    }

    /// Renderuje `out` (próbki przeplatane, `channels` kanałów). `ts` = chwila odtworzenia pierwszej
    /// próbki na urządzeniu (znacznik referencji AEC). Bez alokacji i blokad.
    pub fn render(&mut self, out: &mut [f32], channels: u16, ts: MediaTime) {
        self.apply_commands();
        let ch = usize::from(channels.max(1));
        let frames = out.len() / ch;
        let can_ref = self
            .reference
            .as_ref()
            .is_some_and(|(s, b)| s.slots() >= frames && b.slots() >= 1);
        for frame in out.chunks_exact_mut(ch) {
            let v = self.voice.next_sample(&mut self.reports, &self.shared);
            let e = self.effects.next_sample(&mut self.reports, &self.shared);
            if self.duck_left > 0 {
                self.duck_gain += self.duck_step;
                self.duck_left -= 1;
            }
            let mixed = soft_clip(v * self.duck_gain + e);
            frame.fill(mixed);
            if can_ref && let Some((samples, _)) = self.reference.as_mut() {
                let _ = samples.push(mixed);
            }
        }
        if let Some((_, blocks)) = self.reference.as_mut() {
            if can_ref {
                let _ = blocks.push(RefBlock {
                    ts,
                    len: frames as u32,
                });
            } else if frames > 0 {
                self.shared
                    .dropped_reference
                    .fetch_add(frames as u64, Ordering::Relaxed);
            }
        }
        self.voice.settle(&mut self.reports, &self.shared);
        self.effects.settle(&mut self.reports, &self.shared);
        self.shared
            .rendered
            .fetch_add(frames as u64, Ordering::Relaxed);
        self.shared
            .duck_gain_bits
            .store(self.duck_gain.to_bits(), Ordering::Relaxed);
        self.shared
            .progress
            .publish(self.voice.current, self.voice.rendered);
    }
}

/// Miękkie ograniczenie powyżej 0,9 (bez twardego obcinania przy sumie głosu i earconu).
fn soft_clip(x: f32) -> f32 {
    const KNEE: f32 = 0.9;
    let a = x.abs();
    if a <= KNEE {
        x
    } else {
        let over = (a - KNEE) / (1.0 - KNEE);
        let y = KNEE + (1.0 - KNEE) * over / (1.0 + over);
        y.copysign(x)
    }
}

#[cfg(test)]
mod tests {
    use super::soft_clip;

    #[test]
    fn soft_clip_is_bounded_and_transparent_below_knee() {
        assert_eq!(soft_clip(0.5), 0.5);
        assert_eq!(soft_clip(-0.9), -0.9);
        for x in [1.0f32, 2.0, 10.0, 1e6] {
            assert!(soft_clip(x) <= 1.0 && soft_clip(x) > 0.9);
            assert!(soft_clip(-x) >= -1.0);
        }
    }
}
