//! Atrapa `voice-dsp` (SPEC §Fake): przepuszcza mikrofon (resampling do 16 kHz mono, ramki 10 ms)
//! i zwraca zaprogramowane `aec_confidence` / `speech_prob` według skryptu na osi czasu
//! (wirtualny zegar = znaczniki ramek) — do deterministycznych testów `voice-dialog` / `voice-vad`.
//! Bez skryptu: mowa z energii względem szumu tła, pewność AEC 1 (referencja milczy) albo
//! `reference_confidence` (referencja gra).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::time::Duration;

use voice_audio_contract::gain::{gain_to_db, rms, rms_db};
use voice_audio_contract::{Frame, MediaTime, Resampler};
use voice_dsp_contract::{
    Calibration, Dsp, DspCfg, DspError, DspEvent, DspStats, NoiseFloorTracker, OUTPUT_RATE,
    Processed, SILENCE_DB,
};

const FRAME: usize = (OUTPUT_RATE / 100) as usize;

/// Odcinek skryptu: w `[start, end)` ramki dostają podane wartości.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScriptSegment {
    /// Początek (czas ramki).
    pub start: MediaTime,
    /// Koniec (wyłącznie).
    pub end: MediaTime,
    /// Pewność AEC (jeśli `Some`).
    pub aec_confidence: Option<f32>,
    /// Prawdopodobieństwo mowy (jeśli `Some`).
    pub speech_prob: Option<f32>,
}

/// Atrapa DSP.
#[derive(Debug)]
pub struct FakeDsp {
    cfg: DspCfg,
    resampler: Option<Resampler>,
    buf: Vec<f32>,
    origin: Option<MediaTime>,
    emitted: u64,
    noise: NoiseFloorTracker,
    script: Vec<ScriptSegment>,
    reference: Vec<(MediaTime, MediaTime)>,
    reference_confidence: f32,
    calibration: Option<Calibration>,
    frames: u64,
    events: Vec<DspEvent>,
}

impl Default for FakeDsp {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeDsp {
    /// Nowa atrapa (domyślna konfiguracja).
    pub fn new() -> Self {
        Self {
            cfg: DspCfg::default(),
            resampler: None,
            buf: Vec::new(),
            origin: None,
            emitted: 0,
            noise: NoiseFloorTracker::new(10),
            script: Vec::new(),
            reference: Vec::new(),
            reference_confidence: 0.9,
            calibration: None,
            frames: 0,
            events: Vec::new(),
        }
    }

    /// Dodaje odcinek skryptu.
    pub fn script(&mut self, segment: ScriptSegment) {
        self.script.push(segment);
    }

    /// Pewność AEC, gdy referencja gra, a skrypt nie mówi inaczej.
    pub fn set_reference_confidence(&mut self, c: f32) {
        self.reference_confidence = c.clamp(0.0, 1.0);
    }

    /// Wynik zwracany przez `calibrate`.
    pub fn set_calibration(&mut self, c: Calibration) {
        self.calibration = Some(c);
    }

    fn emit(&mut self, pcm: Vec<f32>) -> Processed {
        let origin = self.origin.unwrap_or(MediaTime::ZERO);
        let ts = origin.plus(Duration::from_nanos(
            MediaTime::from_samples(self.emitted, OUTPUT_RATE).as_nanos(),
        ));
        self.emitted += pcm.len() as u64;
        self.frames += 1;
        let level = rms_db(&pcm);
        let floor = self.noise.update(level);
        let reference_active = self.reference.iter().any(|(a, b)| *a <= ts && ts < *b);
        let mut speech_prob = (((level - floor) - 6.0) / 12.0).clamp(0.0, 1.0);
        let mut aec_confidence = if reference_active {
            self.reference_confidence
        } else {
            1.0
        };
        if let Some(seg) = self.script.iter().find(|s| s.start <= ts && ts < s.end) {
            speech_prob = seg.speech_prob.unwrap_or(speech_prob);
            aec_confidence = seg.aec_confidence.unwrap_or(aec_confidence);
        }
        Processed {
            frame: Frame::mono(pcm, OUTPUT_RATE, ts),
            echo_residual_db: if reference_active { level } else { SILENCE_DB },
            erle_db: 0.0,
            noise_floor_db: floor,
            aec_confidence,
            speech_prob,
            speech_likely: speech_prob >= 0.5,
            reference_active,
        }
    }
}

impl Dsp for FakeDsp {
    fn configure(&mut self, cfg: DspCfg) -> Result<(), DspError> {
        cfg.validate()?;
        self.cfg = cfg;
        Ok(())
    }

    fn config(&self) -> DspCfg {
        self.cfg
    }

    fn push_reference(&mut self, frame: &Frame) {
        if rms(&frame.pcm) > 1e-4 {
            self.reference.push((frame.ts, frame.end_ts()));
            if self.reference.len() > 4_096 {
                self.reference.remove(0);
            }
        }
    }

    fn process(&mut self, mic: &Frame) -> Result<Vec<Processed>, DspError> {
        mic.format
            .validate()
            .map_err(|e| DspError::Format(e.to_string()))?;
        if self.origin.is_none() {
            self.origin = Some(mic.ts);
        }
        let rate = mic.format.sample_rate;
        let r = self
            .resampler
            .get_or_insert_with(|| Resampler::with_zero_crossings(rate, OUTPUT_RATE, 8));
        if r.from_rate() != rate {
            *r = Resampler::with_zero_crossings(rate, OUTPUT_RATE, 8);
        }
        let mut tmp = Vec::new();
        r.process(&mic.to_mono(), &mut tmp);
        self.buf.extend(tmp);
        let mut out = Vec::new();
        while self.buf.len() >= FRAME {
            let pcm: Vec<f32> = self.buf.drain(..FRAME).collect();
            out.push(self.emit(pcm));
        }
        Ok(out)
    }

    fn calibrate(
        &mut self,
        played: &[f32],
        recorded: &[f32],
        _rate: u32,
    ) -> Result<Calibration, DspError> {
        if played.is_empty() || recorded.is_empty() {
            return Err(DspError::Calibration("puste nagranie".into()));
        }
        let c = self
            .calibration
            .ok_or_else(|| DspError::Calibration("atrapa: brak zaprogramowanego wyniku".into()))?;
        self.events.push(DspEvent::Calibrated { calibration: c });
        Ok(c)
    }

    fn stats(&self) -> DspStats {
        DspStats {
            frames: self.frames,
            erle_db: 0.0,
            noise_floor_db: self.noise.floor_db(),
            agc_gain_db: gain_to_db(1.0),
            headphones_likely: false,
            calibrated_loop: self.calibration.map(|c| c.loop_delay),
        }
    }

    fn take_events(&mut self) -> Vec<DspEvent> {
        std::mem::take(&mut self.events)
    }

    fn reset(&mut self) {
        self.resampler = None;
        self.buf.clear();
        self.origin = None;
        self.emitted = 0;
        self.noise.reset();
        self.reference.clear();
    }
}
