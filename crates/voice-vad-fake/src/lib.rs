//! Atrapa `voice-vad`: deterministyczny VAD energetyczny (okna 10 ms) albo zdarzenia ze skryptu
//! adnotacji (przedziały mowy na osi czasu) — wspólny automat decyzji z kontraktu.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::time::Duration;

use voice_audio_contract::{Frame, MediaTime};
use voice_vad_contract::{
    EnergyDetector, Vad, VadCfg, VadError, VadEvent, VadMachine, check_frame,
};

const WINDOW: usize = 160;
const WINDOW_DUR: Duration = Duration::from_millis(10);

/// Atrapa VAD.
#[derive(Debug, Clone)]
pub struct FakeVad {
    cfg: VadCfg,
    machine: VadMachine,
    energy: EnergyDetector,
    buf: Vec<f32>,
    buf_ts: MediaTime,
    last_prob: f32,
    script: Option<Vec<(MediaTime, MediaTime)>>,
}

impl Default for FakeVad {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeVad {
    /// VAD energetyczny z domyślną konfiguracją.
    pub fn new() -> Self {
        let cfg = VadCfg::default();
        Self {
            cfg,
            machine: VadMachine::new(cfg),
            energy: EnergyDetector::new(),
            buf: Vec::with_capacity(WINDOW * 4),
            buf_ts: MediaTime::ZERO,
            last_prob: 0.0,
            script: None,
        }
    }

    /// VAD ze skryptu: mowa dokładnie w podanych przedziałach (audio ignorowane, liczy się czas).
    pub fn scripted(segments: Vec<(MediaTime, MediaTime)>) -> Self {
        Self {
            script: Some(segments),
            ..Self::new()
        }
    }
}

impl Vad for FakeVad {
    fn configure(&mut self, cfg: VadCfg) -> Result<(), VadError> {
        cfg.validate()?;
        self.cfg = cfg;
        self.machine.set_config(cfg);
        Ok(())
    }

    fn config(&self) -> VadCfg {
        self.cfg
    }

    fn push(&mut self, frame: &Frame) -> Result<Vec<VadEvent>, VadError> {
        check_frame(frame)?;
        let expected = self.buf_ts.plus(Duration::from_nanos(
            MediaTime::from_samples(self.buf.len() as u64, 16_000).as_nanos(),
        ));
        if self.buf.is_empty() || expected.as_nanos().abs_diff(frame.ts.as_nanos()) > 20_000_000 {
            self.buf.clear();
            self.buf_ts = frame.ts;
        }
        self.buf.extend_from_slice(&frame.pcm);
        let mut events = Vec::new();
        let mut consumed = 0;
        while self.buf.len() - consumed >= WINDOW {
            let ts = self.buf_ts.plus(WINDOW_DUR * (consumed / WINDOW) as u32);
            let window = &self.buf[consumed..consumed + WINDOW];
            let prob = match &self.script {
                Some(segs) => {
                    if segs.iter().any(|(a, b)| *a <= ts && ts < *b) {
                        1.0
                    } else {
                        0.0
                    }
                }
                None => self.energy.prob(window),
            };
            self.last_prob = prob;
            if let Some(e) = self.machine.step(ts, WINDOW_DUR, prob) {
                events.push(e);
            }
            consumed += WINDOW;
        }
        self.buf.drain(..consumed);
        self.buf_ts = self.buf_ts.plus(WINDOW_DUR * (consumed / WINDOW) as u32);
        Ok(events)
    }

    fn is_speech(&self) -> bool {
        self.machine.is_speech()
    }

    fn last_prob(&self) -> f32 {
        self.last_prob
    }

    fn set_noise_floor(&mut self, db: f32) {
        self.machine.set_noise_floor(db);
        self.energy.set_noise_floor(db);
    }

    fn reset(&mut self) {
        self.machine.reset();
        self.energy.reset();
        self.buf.clear();
        self.last_prob = 0.0;
    }
}
