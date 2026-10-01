//! Potok DSP (48 kHz wewnętrznie, bloki 10 ms): referencja z osi czasu → AEC3 (`sonora`) →
//! RNNoise (`nnnoiseless`) → AGC → 16 kHz (ramki dla VAD/STT) + metryki (ERLE, pewność AEC,
//! szum otoczenia, słuchawki).

use std::time::Duration;

use nnnoiseless::DenoiseState;
use sonora::config::EchoCanceller;
use sonora::{AudioProcessing, Config, StreamConfig};
use voice_audio_contract::gain::{gain_to_db, rms, rms_db};
use voice_audio_contract::{Frame, MediaTime, Resampler};
use voice_dsp_contract::{
    AecMode, Calibration, Dsp, DspCfg, DspError, DspEvent, DspStats, NoiseFloorTracker, NsMode,
    OUTPUT_RATE, Processed, SILENCE_DB,
};

use crate::agc::Agc;
use crate::calibrate::estimate_delay;
use crate::reference::{DSP_RATE, ReferenceTimeline};

const BLOCK: usize = (DSP_RATE / 100) as usize;
const OUT_FRAME: usize = (OUTPUT_RATE / 100) as usize;
/// Skala RNNoise (próbki w zakresie i16).
const RNN_SCALE: f32 = 32_768.0;
/// Poziom, poniżej którego referencja uznawana jest za ciszę (dBFS).
const REF_ACTIVE_DB: f32 = -55.0;
/// Po tylu ramkach aktywnej referencji oceniamy echo / słuchawki (2 s).
const JUDGE_FRAMES: u32 = 200;

fn new_apm() -> AudioProcessing {
    let config = Config {
        echo_canceller: Some(EchoCanceller::default()),
        ..Config::default()
    };
    AudioProcessing::builder()
        .config(config)
        .capture_config(StreamConfig::new(DSP_RATE, 1))
        .render_config(StreamConfig::new(DSP_RATE, 1))
        .build()
}

/// Produkcyjny potok DSP.
pub struct DspPipeline {
    cfg: DspCfg,
    apm: Option<AudioProcessing>,
    ns: Option<Box<DenoiseState<'static>>>,
    agc: Agc,
    mic_rs: Option<Resampler>,
    mic_buf: Vec<f32>,
    /// Indeks (48 kHz) pierwszej próbki `mic_buf`.
    mic_index: Option<u64>,
    reference: ReferenceTimeline,
    out_rs: Resampler,
    out_buf: Vec<f32>,
    out_origin: Option<MediaTime>,
    out_emitted: u64,
    noise: NoiseFloorTracker,
    reported_floor: f32,
    erle_db: f32,
    ref_frames: u32,
    loud_ref_frames: u32,
    quiet_echo_frames: u32,
    echo_high_sent: bool,
    headphones: bool,
    calibrated: Option<Duration>,
    frames: u64,
    events: Vec<DspEvent>,
}

impl std::fmt::Debug for DspPipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DspPipeline")
            .field("cfg", &self.cfg)
            .field("frames", &self.frames)
            .finish()
    }
}

impl DspPipeline {
    /// Potok z konfiguracją (walidowaną).
    pub fn new(cfg: DspCfg) -> Result<Self, DspError> {
        cfg.validate()?;
        let mut p = Self {
            cfg,
            apm: None,
            ns: None,
            agc: Agc::new(cfg.agc_target_db, cfg.whisper_mode),
            mic_rs: None,
            mic_buf: Vec::with_capacity(BLOCK * 4),
            mic_index: None,
            reference: ReferenceTimeline::default(),
            out_rs: Resampler::new(DSP_RATE, OUTPUT_RATE),
            out_buf: Vec::with_capacity(OUT_FRAME * 4),
            out_origin: None,
            out_emitted: 0,
            noise: NoiseFloorTracker::new(10),
            reported_floor: SILENCE_DB,
            erle_db: 0.0,
            ref_frames: 0,
            loud_ref_frames: 0,
            quiet_echo_frames: 0,
            echo_high_sent: false,
            headphones: false,
            calibrated: None,
            frames: 0,
            events: Vec::new(),
        };
        p.apply_modes();
        Ok(p)
    }

    fn apply_modes(&mut self) {
        let aec = matches!(self.cfg.aec, AecMode::OwnReference | AecMode::Loopback);
        self.apm = aec.then(new_apm);
        self.ns = match self.cfg.ns {
            NsMode::Off => None,
            NsMode::RnNoise => Some(DenoiseState::new()),
            NsMode::DeepFilter => {
                self.events.push(DspEvent::ModeFallback {
                    from: "deep_filter".into(),
                    to: "rn_noise".into(),
                    reason: "DeepFilterNet3 niedostępny w v0 — używam RNNoise".into(),
                });
                Some(DenoiseState::new())
            }
        };
        self.agc = Agc::new(self.cfg.agc_target_db, self.cfg.whisper_mode);
    }

    fn reference_offset(&self) -> i64 {
        let margin = i64::from(self.cfg.reference_margin_ms) * i64::from(DSP_RATE) / 1000;
        let cal = self.calibrated.map_or(0, |d| {
            (d.as_micros() as i64) * i64::from(DSP_RATE) / 1_000_000
        });
        margin - cal
    }

    /// Przetwarza jeden blok 10 ms (48 kHz) zaczynający się w indeksie `index`.
    fn process_block(&mut self, index: u64, mic: &[f32], out: &mut Vec<Processed>) {
        let mut reference = [0.0f32; BLOCK];
        self.reference
            .read(index as i64 + self.reference_offset(), &mut reference);
        let ref_db = rms_db(&reference);
        let reference_active = ref_db > REF_ACTIVE_DB;
        let mut block = [0.0f32; BLOCK];
        block.copy_from_slice(mic);
        let mic_power = rms(mic).powi(2);
        if let Some(apm) = self.apm.as_mut() {
            let mut sink = [0.0f32; BLOCK];
            let _ = apm.process_render_f32(&[&reference[..]], &mut [&mut sink[..]]);
            let _ = apm.set_stream_delay_ms(self.cfg.reference_margin_ms as i32);
            let _ = apm.process_capture_f32(&[mic], &mut [&mut block[..]]);
        }
        let aec_power = rms(&block).powi(2);
        let level_db = gain_to_db(aec_power.sqrt());
        let floor = self.noise.update(level_db);
        self.update_echo_metrics(reference_active, ref_db, mic_power, aec_power);
        let speech_prob = match self.ns.as_mut() {
            Some(ns) => {
                let input: Vec<f32> = block.iter().map(|s| s * RNN_SCALE).collect();
                let mut denoised = [0.0f32; BLOCK];
                let p = ns.process_frame(&mut denoised, &input);
                for (b, d) in block.iter_mut().zip(denoised) {
                    *b = d / RNN_SCALE;
                }
                p
            }
            None => (((level_db - floor) - 6.0) / 12.0).clamp(0.0, 1.0),
        };
        if self.cfg.agc {
            self.agc.process(&mut block, speech_prob >= 0.5);
        }
        self.out_rs.process(&block, &mut self.out_buf);
        let echo_residual_db = if reference_active {
            level_db
        } else {
            SILENCE_DB
        };
        let aec_confidence = if !reference_active || self.headphones {
            1.0
        } else {
            match self.cfg.aec {
                AecMode::OwnReference | AecMode::Loopback => (self.erle_db / 20.0).clamp(0.0, 1.0),
                // AEC systemu Windows — jakości nie mierzymy, zakładamy umiarkowaną.
                AecMode::Communications => 0.6,
                AecMode::Off => 0.0,
            }
        };
        while self.out_buf.len() >= OUT_FRAME {
            let pcm: Vec<f32> = self.out_buf.drain(..OUT_FRAME).collect();
            let origin = self.out_origin.unwrap_or(MediaTime::ZERO);
            let ts = origin.plus(Duration::from_nanos(
                MediaTime::from_samples(self.out_emitted, OUTPUT_RATE).as_nanos(),
            ));
            self.out_emitted += OUT_FRAME as u64;
            out.push(Processed {
                frame: Frame::mono(pcm, OUTPUT_RATE, ts),
                echo_residual_db,
                erle_db: self.erle_db,
                noise_floor_db: floor,
                aec_confidence,
                speech_prob,
                speech_likely: speech_prob >= 0.5,
                reference_active,
            });
        }
        if (floor - self.reported_floor).abs() >= 6.0 {
            self.reported_floor = floor;
            self.events.push(DspEvent::NoiseChanged { floor_db: floor });
        }
        self.frames += 1;
    }

    fn update_echo_metrics(&mut self, active: bool, ref_db: f32, mic_power: f32, aec_power: f32) {
        if !active {
            return;
        }
        let inst = (10.0 * (mic_power.max(1e-12) / aec_power.max(1e-12)).log10()).clamp(0.0, 60.0);
        self.erle_db = 0.95 * self.erle_db + 0.05 * inst;
        self.ref_frames = self.ref_frames.saturating_add(1);
        // Słuchawki: przy wyraźnej referencji (> −40 dBFS) mikrofon co najmniej 30 dB ciszej.
        if ref_db > -40.0 {
            self.loud_ref_frames = self.loud_ref_frames.saturating_add(1);
            let mic_db = 10.0 * mic_power.max(1e-12).log10();
            if mic_db < ref_db - 30.0 {
                self.quiet_echo_frames = self.quiet_echo_frames.saturating_add(1);
            }
            if self.loud_ref_frames == JUDGE_FRAMES {
                let likely = self.quiet_echo_frames * 10 >= JUDGE_FRAMES * 9;
                if likely != self.headphones {
                    self.headphones = likely;
                    self.events.push(DspEvent::Headphones { likely });
                }
            }
        }
        if self.ref_frames >= JUDGE_FRAMES
            && !self.headphones
            && !self.echo_high_sent
            && self.erle_db < 6.0
        {
            self.echo_high_sent = true;
            self.events.push(DspEvent::EchoHigh {
                erle_db: self.erle_db,
            });
        }
    }
}

impl Dsp for DspPipeline {
    fn configure(&mut self, cfg: DspCfg) -> Result<(), DspError> {
        cfg.validate()?;
        let modes_changed = (cfg.aec, cfg.ns) != (self.cfg.aec, self.cfg.ns);
        self.cfg = cfg;
        if modes_changed {
            self.apply_modes();
        } else {
            self.agc = Agc::new(cfg.agc_target_db, cfg.whisper_mode);
        }
        Ok(())
    }

    fn config(&self) -> DspCfg {
        self.cfg
    }

    fn push_reference(&mut self, frame: &Frame) {
        // Także bez AEC: metryki (aktywność referencji, słuchawki, `echo_high`).
        self.reference.push(frame);
    }

    fn process(&mut self, mic: &Frame) -> Result<Vec<Processed>, DspError> {
        mic.format
            .validate()
            .map_err(|e| DspError::Format(e.to_string()))?;
        let rate = mic.format.sample_rate;
        let index = mic.ts.to_samples(DSP_RATE);
        let expected = self.mic_index.map(|i| i + self.mic_buf.len() as u64);
        // Nieciągłość (> 20 ms) → nowy strumień (zmiana urządzenia, przerwa w przechwytywaniu).
        if expected.is_none_or(|e| e.abs_diff(index) > u64::from(DSP_RATE / 50)) {
            self.mic_buf.clear();
            self.mic_index = Some(index);
            self.out_origin = Some(mic.ts);
            self.out_emitted = 0;
            self.out_buf.clear();
            self.out_rs.reset();
        }
        let mono = mic.to_mono();
        if rate == DSP_RATE {
            self.mic_buf.extend_from_slice(&mono);
        } else {
            let r = self
                .mic_rs
                .get_or_insert_with(|| Resampler::new(rate, DSP_RATE));
            if r.from_rate() != rate {
                *r = Resampler::new(rate, DSP_RATE);
            }
            r.process(&mono, &mut self.mic_buf);
        }
        let mut out = Vec::new();
        let mut consumed = 0;
        while self.mic_buf.len() - consumed >= BLOCK {
            let start = self.mic_index.unwrap_or(index) + consumed as u64;
            let block: Vec<f32> = self.mic_buf[consumed..consumed + BLOCK].to_vec();
            self.process_block(start, &block, &mut out);
            consumed += BLOCK;
        }
        self.mic_buf.drain(..consumed);
        self.mic_index = self.mic_index.map(|i| i + consumed as u64);
        Ok(out)
    }

    fn calibrate(
        &mut self,
        played: &[f32],
        recorded: &[f32],
        rate: u32,
    ) -> Result<Calibration, DspError> {
        let c = estimate_delay(played, recorded, rate)?;
        self.calibrated = Some(c.loop_delay);
        self.events.push(DspEvent::Calibrated { calibration: c });
        Ok(c)
    }

    fn stats(&self) -> DspStats {
        DspStats {
            frames: self.frames,
            erle_db: self.erle_db,
            noise_floor_db: self.noise.floor_db(),
            agc_gain_db: self.agc.gain_db(),
            headphones_likely: self.headphones,
            calibrated_loop: self.calibrated,
        }
    }

    fn take_events(&mut self) -> Vec<DspEvent> {
        std::mem::take(&mut self.events)
    }

    fn reset(&mut self) {
        let (cfg, calibrated) = (self.cfg, self.calibrated);
        if let Ok(mut fresh) = Self::new(cfg) {
            fresh.calibrated = calibrated;
            fresh.events = std::mem::take(&mut self.events);
            *self = fresh;
        }
    }
}
