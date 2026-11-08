//! Deterministyczne modele słów wywoławczych do testów i CI (bez ONNX):
//! - [`ToneScorer`] — każda etykieta ma „podpis” tonu (fraza atrapy = ton o danej częstotliwości);
//!   wynik = udział energii tonu w oknie 80 ms (Goertzel). Mowa syntetyczna (harmoniczne 210 Hz)
//!   daje wyniki bliskie zera, więc działa jako tło do FAR;
//! - [`ScriptedScorer`] — wyniki z listy `(czas, etykieta, wynik)` niezależnie od audio (czas od
//!   ostatniego `reset`, tj. od początku segmentu; z `vad_gate = false` — od startu strumienia).

use voice_wake_contract::{KeywordScorer, KwsScores, WakeError};

/// Okno modelu atrapy (80 ms @ 16 kHz).
pub const WINDOW: usize = 1_280;

/// Częstotliwości podpisów wbudowanych fraz (między harmonicznymi 210 Hz mowy syntetycznej).
pub const BUILTIN_TONES: [(&str, f32); 4] = [
    ("hej alfa", 735.0),
    ("hej beta", 945.0),
    ("hej gama", 1_155.0),
    ("hej delta", 1_365.0),
];

/// Model atrapy na tonach.
#[derive(Debug, Clone)]
pub struct ToneScorer {
    labels: Vec<String>,
    tones: Vec<f32>,
    buf: Vec<f32>,
    t_ms: u64,
    fed_samples: u64,
}

impl Default for ToneScorer {
    fn default() -> Self {
        Self::builtin()
    }
}

impl ToneScorer {
    /// Etykiety i częstotliwości podpisów.
    pub fn new(tones: &[(&str, f32)]) -> Self {
        Self {
            labels: tones.iter().map(|(l, _)| (*l).to_owned()).collect(),
            tones: tones.iter().map(|(_, f)| *f).collect(),
            buf: Vec::with_capacity(WINDOW),
            t_ms: 0,
            fed_samples: 0,
        }
    }

    /// „Hej Alfa/Beta/Gama/Delta” → [`BUILTIN_TONES`].
    pub fn builtin() -> Self {
        Self::new(&BUILTIN_TONES)
    }

    /// Ile próbek dostał model (testy prywatności: audio tylko przy otwartej bramce).
    pub fn fed_samples(&self) -> u64 {
        self.fed_samples
    }

    fn score(&self, w: &[f32], hz: f32) -> f32 {
        // Goertzel: moc prążka `hz` względem energii okna (1,0 = czysty ton).
        let coeff = 2.0 * (2.0 * std::f64::consts::PI * f64::from(hz) / 16_000.0).cos();
        let (mut s1, mut s2, mut e) = (0.0f64, 0.0f64, 0.0f64);
        for x in w {
            let x = f64::from(*x);
            let s0 = x + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
            e += x * x;
        }
        let power = s1 * s1 + s2 * s2 - coeff * s1 * s2;
        let ratio = 2.0 * power / w.len() as f64 / e.max(1e-12);
        ratio.clamp(0.0, 1.0) as f32
    }
}

impl KeywordScorer for ToneScorer {
    fn labels(&self) -> &[String] {
        &self.labels
    }

    fn push(&mut self, samples: &[f32]) -> Result<Vec<KwsScores>, WakeError> {
        self.fed_samples += samples.len() as u64;
        self.buf.extend_from_slice(samples);
        let mut out = Vec::new();
        while self.buf.len() >= WINDOW {
            let w: Vec<f32> = self.buf.drain(..WINDOW).collect();
            self.t_ms += 80;
            out.push(KwsScores {
                at_ms: self.t_ms,
                scores: self.tones.iter().map(|f| self.score(&w, *f)).collect(),
            });
        }
        Ok(out)
    }

    fn reset(&mut self) {
        self.buf.clear();
        self.t_ms = 0;
    }
}

/// Model atrapy ze skryptem wyników.
#[derive(Debug, Clone)]
pub struct ScriptedScorer {
    labels: Vec<String>,
    script: Vec<(u64, usize, f32)>,
    samples: u64,
    step_ms: u64,
    next_ms: u64,
}

impl ScriptedScorer {
    /// Etykiety, krok (ms) i skrypt `(czas od startu strumienia, etykieta, wynik)`.
    pub fn new(labels: &[&str], step_ms: u64, script: Vec<(u64, usize, f32)>) -> Self {
        Self {
            labels: labels.iter().map(|l| (*l).to_owned()).collect(),
            script,
            samples: 0,
            step_ms: step_ms.max(10),
            next_ms: step_ms.max(10),
        }
    }
}

impl KeywordScorer for ScriptedScorer {
    fn labels(&self) -> &[String] {
        &self.labels
    }

    fn push(&mut self, samples: &[f32]) -> Result<Vec<KwsScores>, WakeError> {
        self.samples += samples.len() as u64;
        let now = self.samples / 16;
        let mut out = Vec::new();
        while self.next_ms <= now {
            let at = self.next_ms;
            let mut scores = vec![0.0; self.labels.len()];
            for (t, l, v) in &self.script {
                if *t <= at
                    && at < t + self.step_ms
                    && let Some(s) = scores.get_mut(*l)
                {
                    *s = *v;
                }
            }
            out.push(KwsScores { at_ms: at, scores });
            self.next_ms += self.step_ms;
        }
        Ok(out)
    }

    fn reset(&mut self) {
        self.samples = 0;
        self.next_ms = self.step_ms;
    }
}
