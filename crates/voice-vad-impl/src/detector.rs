//! [`SileroVad`] — implementacja `Vad`: okna 32 ms, Silero (gdy model załadowany) albo energia
//! (zapas), wspólny automat decyzji z kontraktu.

use std::time::Duration;

use voice_audio_contract::{Frame, MediaTime};
use voice_vad_contract::{
    EnergyDetector, SILERO_WINDOW, VAD_RATE, Vad, VadCfg, VadEngine, VadError, VadEvent,
    VadMachine, check_frame,
};

use crate::residency::LeaseGuard;
use crate::silero::SileroModel;

const WINDOW_DUR: Duration = Duration::from_millis(32);

/// VAD produkcyjny.
#[derive(Debug)]
pub struct SileroVad {
    cfg: VadCfg,
    machine: VadMachine,
    model: Option<SileroModel>,
    energy: EnergyDetector,
    buf: Vec<f32>,
    buf_ts: MediaTime,
    last_prob: f32,
    model_errors: u64,
    _lease: Option<LeaseGuard>,
}

impl SileroVad {
    /// VAD z modelem Silero (`None` = detektor energii).
    pub fn new(cfg: VadCfg, model: Option<SileroModel>) -> Result<Self, VadError> {
        cfg.validate()?;
        Ok(Self {
            cfg,
            machine: VadMachine::new(cfg),
            model,
            energy: EnergyDetector::new(),
            buf: Vec::with_capacity(SILERO_WINDOW * 4),
            buf_ts: MediaTime::ZERO,
            last_prob: 0.0,
            model_errors: 0,
            _lease: None,
        })
    }

    /// Przypina dzierżawę rezydencji (zwalniana razem z VAD).
    pub(crate) fn with_lease(mut self, lease: Option<LeaseGuard>) -> Self {
        self._lease = lease;
        self
    }

    /// Silnik w użyciu.
    pub fn engine(&self) -> VadEngine {
        if self.model.is_some() && self.cfg.engine != VadEngine::Energy {
            VadEngine::Silero
        } else {
            VadEngine::Energy
        }
    }

    /// Liczba błędów inferencji (okno liczone energią).
    pub fn model_errors(&self) -> u64 {
        self.model_errors
    }

    fn prob(&mut self, window: &[f32]) -> f32 {
        if self.cfg.engine != VadEngine::Energy
            && let Some(m) = self.model.as_mut()
        {
            match m.infer(window) {
                Ok(p) => return p,
                Err(_) => self.model_errors += 1,
            }
        }
        self.energy.prob(window)
    }
}

impl Vad for SileroVad {
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
            MediaTime::from_samples(self.buf.len() as u64, VAD_RATE).as_nanos(),
        ));
        if self.buf.is_empty() || expected.as_nanos().abs_diff(frame.ts.as_nanos()) > 20_000_000 {
            self.buf.clear();
            self.buf_ts = frame.ts;
        }
        self.buf.extend_from_slice(&frame.pcm);
        let mut events = Vec::new();
        let mut consumed = 0;
        while self.buf.len() - consumed >= SILERO_WINDOW {
            let ts = self
                .buf_ts
                .plus(WINDOW_DUR * (consumed / SILERO_WINDOW) as u32);
            let window: Vec<f32> = self.buf[consumed..consumed + SILERO_WINDOW].to_vec();
            let prob = self.prob(&window);
            self.last_prob = prob;
            if let Some(e) = self.machine.step(ts, WINDOW_DUR, prob) {
                events.push(e);
            }
            consumed += SILERO_WINDOW;
        }
        self.buf.drain(..consumed);
        self.buf_ts = self
            .buf_ts
            .plus(WINDOW_DUR * (consumed / SILERO_WINDOW) as u32);
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
        if let Some(m) = self.model.as_mut() {
            m.reset();
        }
    }
}
