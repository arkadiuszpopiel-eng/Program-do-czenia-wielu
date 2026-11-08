//! Reguła echa (uzupełnienie AEC, VOICE.md §11): w trakcie mowy agentki ramka mikrofonu trafia do
//! VAD jako mowa tylko wtedy, gdy jest wyraźnie głośniejsza od przewidywanego resztkowego echa
//! własnego TTS. Przewidywanie to szczyt referencji (to, co zagrało) w oknie opóźnienia pętli plus
//! sprzężenie głośnik→mikrofon minus ERLE z DSP. Sprzężenie jest mierzone na ramkach uznanych za
//! echo (wolno w górę, wolno w dół), start z konfiguracji (ostrożnie: −6 dB). Klasyczny detektor
//! „podwójnej mowy” (Geigel) w dziedzinie poziomów — deterministyczny i tani.

use std::collections::VecDeque;

use voice_audio_contract::Frame;
use voice_audio_contract::gain::rms_db;
use voice_pipeline_contract::EchoGateCfg;

/// Najszybsze podniesienie szacunku sprzężenia na ramkę (dB).
const RISE_STEP_DB: f32 = 0.5;
/// Opadanie szacunku sprzężenia na ramkę, gdy echo jest wyraźnie cichsze (dB).
const FALL_STEP_DB: f32 = 0.05;

/// Reguła echa.
#[derive(Debug, Clone)]
pub struct EchoGate {
    cfg: EchoGateCfg,
    refs: VecDeque<(u64, f32)>,
    coupling_db: f32,
    gated: u64,
}

impl EchoGate {
    /// Nowa reguła.
    pub fn new(cfg: EchoGateCfg) -> Self {
        Self {
            cfg,
            refs: VecDeque::new(),
            coupling_db: cfg.initial_coupling_db,
            gated: 0,
        }
    }

    /// Bieżący szacunek sprzężenia (dB).
    pub fn coupling_db(&self) -> f32 {
        self.coupling_db
    }

    /// Ramki uznane za echo (od startu).
    pub fn gated(&self) -> u64 {
        self.gated
    }

    /// Referencja: to, co zagrało (czas odtworzenia), w blokach 10 ms.
    pub fn push_reference(&mut self, frame: &Frame) {
        let mono = frame.to_mono();
        let block = (frame.format.sample_rate as usize / 100).max(1);
        for (i, b) in mono.chunks(block).enumerate() {
            let ts = frame.ts.as_ms() + 10 * i as u64;
            self.refs.push_back((ts, rms_db(b)));
        }
        let keep_from = self.refs.back().map_or(0, |(t, _)| {
            t.saturating_sub(u64::from(self.cfg.tail_ms) + 1_000)
        });
        while self.refs.front().is_some_and(|(t, _)| *t < keep_from) {
            self.refs.pop_front();
        }
    }

    fn reference_peak(&self, ts_ms: u64) -> Option<f32> {
        let from = ts_ms.saturating_sub(u64::from(self.cfg.tail_ms));
        self.refs
            .iter()
            .filter(|(t, _)| *t >= from && *t <= ts_ms + 10)
            .map(|(_, db)| *db)
            .reduce(f32::max)
            .filter(|db| *db > self.cfg.reference_floor_db)
    }

    /// Czy ramka mikrofonu (poziom po DSP) może być mową użytkownika.
    pub fn near_end(&mut self, ts_ms: u64, mic_db: f32, erle_db: f32, aec_confidence: f32) -> bool {
        if !self.cfg.enabled {
            return true;
        }
        let Some(peak) = self.reference_peak(ts_ms) else {
            return true;
        };
        let erle = erle_db.max(0.0);
        let predicted = peak + self.coupling_db - erle;
        let near = mic_db >= predicted + self.cfg.margin_db
            && aec_confidence >= self.cfg.min_aec_confidence;
        if !near {
            self.gated += 1;
            let observed = mic_db - peak + erle;
            if observed > self.coupling_db {
                self.coupling_db += ((observed - self.coupling_db) * 0.1).min(RISE_STEP_DB);
            } else if observed < self.coupling_db - 3.0 {
                self.coupling_db -= FALL_STEP_DB;
            }
            self.coupling_db = self.coupling_db.clamp(-60.0, 6.0);
        }
        near
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_audio_contract::MediaTime;
    use voice_audio_contract::gain::db_to_gain;

    fn tone(db: f32, ms: u64, ts_ms: u64) -> Frame {
        let a = db_to_gain(db) * std::f32::consts::SQRT_2;
        let n = 48 * ms as usize;
        let pcm: Vec<f32> = (0..n).map(|i| a * (i as f32 * 0.05).sin()).collect();
        Frame::mono(pcm, 48_000, MediaTime::from_ms(ts_ms))
    }

    #[test]
    fn echo_is_gated_and_user_passes_and_coupling_adapts() {
        let mut g = EchoGate::new(EchoGateCfg::default());
        assert!(g.near_end(0, -30.0, 0.0, 1.0), "bez referencji zawsze mowa");
        // Referencja napływa strumieniowo (10 ms), echo −12 dB względem niej: poniżej progu.
        for t in (0..1_500).step_by(10) {
            g.push_reference(&tone(-20.0, 10, t));
            if t >= 100 {
                assert!(!g.near_end(t, -32.0, 0.0, 0.9), "t = {t}");
            }
        }
        assert!(
            g.coupling_db() < -6.0,
            "sprzężenie zmierzone: {}",
            g.coupling_db()
        );
        // Mowa użytkownika wyraźnie ponad echem.
        assert!(g.near_end(1_490, -12.0, 0.0, 0.9));
        // Niska pewność AEC → echo.
        assert!(!g.near_end(1_490, -12.0, 0.0, 0.1));
        // ERLE z DSP obniża przewidywanie.
        assert!(g.near_end(1_490, -24.0, 20.0, 0.9));
        assert!(g.gated() > 100);
        // Po ogonie referencji reguła nieaktywna.
        assert!(g.near_end(2_400, -40.0, 0.0, 0.9));
        let mut off = EchoGate::new(EchoGateCfg {
            enabled: false,
            ..EchoGateCfg::default()
        });
        off.push_reference(&tone(-20.0, 100, 0));
        assert!(off.near_end(50, -60.0, 0.0, 0.0));
    }

    #[test]
    fn loud_echo_raises_coupling_slowly() {
        let mut g = EchoGate::new(EchoGateCfg::default());
        g.push_reference(&tone(-20.0, 1_000, 0));
        let before = g.coupling_db();
        assert!(!g.near_end(100, -23.0, 0.0, 1.0));
        assert!(g.coupling_db() > before && g.coupling_db() <= before + RISE_STEP_DB);
        g.push_reference(&tone(-20.0, 10_000, 1_000));
        assert!(g.refs.len() < 300, "historia przycięta: {}", g.refs.len());
    }
}
