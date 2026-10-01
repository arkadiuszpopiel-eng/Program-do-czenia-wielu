//! Deterministyczny automat decyzji VAD (wspólny dla Silero i detektora energii, `-impl` i `-fake`):
//! próg z histerezą, minimalny czas mowy i ciszy, próg adaptacyjny względem szumu otoczenia
//! (nigdy poniżej `min_threshold`).

use std::time::Duration;

use voice_audio_contract::MediaTime;

use crate::{VadCfg, VadEvent};

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Silence,
    /// Kandydat na mowę od `start`, uzbierane `ms` mowy.
    Rising {
        start: MediaTime,
        ms: u32,
        max_prob: f32,
    },
    /// Mowa od `start`.
    Speech {
        start: MediaTime,
    },
    /// Cisza w trakcie mowy od `since` (jeszcze nie dość długa).
    Falling {
        start: MediaTime,
        since: MediaTime,
        ms: u32,
    },
}

/// Automat decyzji: wejście = prawdopodobieństwo mowy okna, wyjście = zdarzenia.
#[derive(Debug, Clone)]
pub struct VadMachine {
    cfg: VadCfg,
    state: State,
    noise_floor_db: f32,
}

impl VadMachine {
    /// Nowy automat (cisza).
    pub fn new(cfg: VadCfg) -> Self {
        Self {
            cfg,
            state: State::Silence,
            noise_floor_db: -70.0,
        }
    }

    /// Zmienia konfigurację (stan zostaje).
    pub fn set_config(&mut self, cfg: VadCfg) {
        self.cfg = cfg;
    }

    /// Szum otoczenia (dBFS) z `voice-dsp` — podstawa progu adaptacyjnego.
    pub fn set_noise_floor(&mut self, db: f32) {
        self.noise_floor_db = db;
    }

    /// Bieżący próg wejścia w mowę: bazowy + do 0,2 przy szumie −60 → −30 dBFS, w granicach
    /// `[min_threshold, max_threshold]`.
    pub fn threshold(&self) -> f32 {
        let base = self.cfg.threshold;
        let t = if self.cfg.adaptive {
            base + 0.2 * ((self.noise_floor_db + 60.0) / 30.0).clamp(0.0, 1.0)
        } else {
            base
        };
        t.clamp(
            self.cfg.min_threshold,
            self.cfg.max_threshold.max(self.cfg.min_threshold),
        )
    }

    /// Próg wyjścia z mowy (histereza).
    pub fn neg_threshold(&self) -> f32 {
        (self.threshold() - self.cfg.hysteresis).max(0.01)
    }

    /// Czy trwa mowa (po decyzji `SpeechStart`, przed `SpeechEnd`).
    pub fn is_speech(&self) -> bool {
        matches!(self.state, State::Speech { .. } | State::Falling { .. })
    }

    /// Reset (zmiana urządzenia).
    pub fn reset(&mut self) {
        self.state = State::Silence;
    }

    /// Przetwarza okno `[ts, ts + dur)` o prawdopodobieństwie mowy `prob`.
    pub fn step(&mut self, ts: MediaTime, dur: Duration, prob: f32) -> Option<VadEvent> {
        let ms = u32::try_from(dur.as_millis()).unwrap_or(u32::MAX);
        let (on, off) = (self.threshold(), self.neg_threshold());
        match self.state {
            State::Silence => {
                if prob >= on {
                    self.state = State::Rising {
                        start: ts,
                        ms: 0,
                        max_prob: 0.0,
                    };
                    return self.step_rising(ts, ms, prob);
                }
                None
            }
            State::Rising { .. } => {
                if prob >= on {
                    self.step_rising(ts, ms, prob)
                } else {
                    self.state = State::Silence;
                    None
                }
            }
            State::Speech { start } => {
                if prob < off {
                    self.state = State::Falling {
                        start,
                        since: ts,
                        ms: 0,
                    };
                    return self.step_falling(ms);
                }
                None
            }
            State::Falling { start, .. } => {
                if prob >= off {
                    self.state = State::Speech { start };
                    None
                } else {
                    self.step_falling(ms)
                }
            }
        }
    }

    fn step_rising(&mut self, _ts: MediaTime, ms: u32, prob: f32) -> Option<VadEvent> {
        if let State::Rising {
            start,
            ms: acc,
            max_prob,
        } = self.state
        {
            let acc = acc + ms;
            let max_prob = max_prob.max(prob);
            if acc >= u32::from(self.cfg.min_speech_ms) {
                self.state = State::Speech { start };
                return Some(VadEvent::SpeechStart {
                    ts: start,
                    prob: max_prob,
                });
            }
            self.state = State::Rising {
                start,
                ms: acc,
                max_prob,
            };
        }
        None
    }

    fn step_falling(&mut self, ms: u32) -> Option<VadEvent> {
        if let State::Falling {
            start,
            since,
            ms: acc,
        } = self.state
        {
            let acc = acc + ms;
            if acc >= u32::from(self.cfg.min_silence_ms) {
                self.state = State::Silence;
                return Some(VadEvent::SpeechEnd {
                    ts: since,
                    duration: since.since(start),
                });
            }
            self.state = State::Falling {
                start,
                since,
                ms: acc,
            };
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(m: &mut VadMachine, probs: &[f32], win_ms: u64) -> Vec<VadEvent> {
        probs
            .iter()
            .enumerate()
            .filter_map(|(i, &p)| {
                m.step(
                    MediaTime::from_ms(i as u64 * win_ms),
                    Duration::from_millis(win_ms),
                    p,
                )
            })
            .collect()
    }

    #[test]
    fn hysteresis_and_minimum_durations() {
        let cfg = VadCfg {
            min_speech_ms: 60,
            min_silence_ms: 100,
            adaptive: false,
            ..VadCfg::default()
        };
        let mut m = VadMachine::new(cfg);
        // Krótki trzask (1 okno) nie jest mową; potem mowa 5 okien, dołek 0,4 (> próg wyjścia),
        // krótka pauza (2 okna) i koniec po 4 oknach ciszy.
        let probs = [
            0.0, 0.9, 0.0, 0.8, 0.9, 0.9, 0.4, 0.9, 0.1, 0.1, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        let ev = run(&mut m, &probs, 32);
        assert_eq!(ev.len(), 2, "{ev:?}");
        assert_eq!(
            ev[0],
            VadEvent::SpeechStart {
                ts: MediaTime::from_ms(96),
                prob: 0.9
            }
        );
        match ev[1] {
            VadEvent::SpeechEnd { ts, duration } => {
                assert_eq!(ts, MediaTime::from_ms(11 * 32));
                assert_eq!(duration, Duration::from_millis(8 * 32));
            }
            _ => panic!("{ev:?}"),
        }
        assert!(!m.is_speech());
    }

    #[test]
    fn adaptive_threshold_never_below_minimum() {
        let mut m = VadMachine::new(VadCfg::default());
        m.set_noise_floor(-80.0);
        assert!((m.threshold() - 0.5).abs() < 1e-6);
        m.set_noise_floor(-30.0);
        assert!((m.threshold() - 0.7).abs() < 1e-6);
        let low = VadCfg {
            threshold: 0.1,
            min_threshold: 0.3,
            adaptive: false,
            ..VadCfg::default()
        };
        m.set_config(low);
        assert!((m.threshold() - 0.3).abs() < 1e-6);
        assert!(m.neg_threshold() > 0.0);
        // W szumie ta sama prawdopodobność (0,6) nie startuje mowy.
        let mut noisy = VadMachine::new(VadCfg::default());
        noisy.set_noise_floor(-30.0);
        assert!(run(&mut noisy, &[0.6; 10], 32).is_empty());
        noisy.reset();
        assert!(!noisy.is_speech());
    }
}
