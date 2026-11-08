//! Klasyfikator słów wywoławczych na cechach log-mel liczonych w Rust: strumień ramek 10 ms
//! (`FbankStream`), okno ostatnich `frames` ramek (na starcie segmentu dopełnione ciszą),
//! wynik co `step_frames` ramek.

use std::collections::VecDeque;

use voice_dsp_contract::{Fbank, FbankCfg, FbankStream};
use voice_wake_contract::{KeywordScorer, KwsScores, WakeError};

use super::manifest::{Activation, Layout};
use super::onnx::{Plan, err, sigmoid, softmax};

/// Parametry klasyfikatora.
pub(crate) struct LogMelSpec {
    pub n_mels: usize,
    pub frames: usize,
    pub step_frames: usize,
    pub layout: Layout,
    pub activation: Activation,
    pub background_index: Option<usize>,
}

/// Model `log_mel`.
pub(crate) struct LogMelScorer {
    labels: Vec<String>,
    spec: LogMelSpec,
    plan: Plan,
    stream: FbankStream,
    window: VecDeque<Vec<f32>>,
    silence: f32,
    since_step: usize,
    frames_total: u64,
}

impl LogMelScorer {
    pub(crate) fn new(
        labels: Vec<String>,
        spec: LogMelSpec,
        bytes: &[u8],
    ) -> Result<Self, WakeError> {
        let cfg = FbankCfg::kaldi(spec.n_mels);
        let fbank = Fbank::new(cfg).map_err(err)?;
        let shape = match spec.layout {
            Layout::Btf => vec![1, spec.frames, spec.n_mels],
            Layout::Bft => vec![1, spec.n_mels, spec.frames],
            Layout::B1tf => vec![1, 1, spec.frames, spec.n_mels],
        };
        let plan = Plan::load(bytes, &shape)?;
        let mut s = Self {
            labels,
            silence: cfg.log_floor.ln(),
            stream: FbankStream::new(fbank),
            window: VecDeque::with_capacity(spec.frames),
            plan,
            spec,
            since_step: 0,
            frames_total: 0,
        };
        s.reset();
        Ok(s)
    }

    fn input(&self) -> Vec<f32> {
        let (t, m) = (self.spec.frames, self.spec.n_mels);
        match self.spec.layout {
            Layout::Btf | Layout::B1tf => self.window.iter().flatten().copied().collect(),
            Layout::Bft => (0..m)
                .flat_map(|j| (0..t).map(move |i| (i, j)))
                .map(|(i, j)| {
                    self.window
                        .get(i)
                        .and_then(|f| f.get(j))
                        .copied()
                        .unwrap_or(0.0)
                })
                .collect(),
        }
    }

    fn scores(&self, raw: &[f32]) -> Vec<f32> {
        let probs = match self.spec.activation {
            Activation::Sigmoid => raw.iter().map(|x| sigmoid(*x)).collect(),
            Activation::Softmax => softmax(raw),
            Activation::None => raw.to_vec(),
        };
        let keep = |i: &usize| Some(*i) != self.spec.background_index;
        (0..probs.len())
            .filter(keep)
            .map(|i| probs[i].clamp(0.0, 1.0))
            .take(self.labels.len())
            .collect()
    }
}

impl KeywordScorer for LogMelScorer {
    fn labels(&self) -> &[String] {
        &self.labels
    }

    fn push(&mut self, samples: &[f32]) -> Result<Vec<KwsScores>, WakeError> {
        let mut out = Vec::new();
        for frame in self.stream.push(samples) {
            self.frames_total += 1;
            if self.window.len() == self.spec.frames {
                self.window.pop_front();
            }
            self.window.push_back(frame);
            self.since_step += 1;
            if self.since_step >= self.spec.step_frames {
                self.since_step = 0;
                let raw = self.plan.run(&self.input())?;
                out.push(KwsScores {
                    // Koniec okna: ramka i trwa 25 ms od i·10 ms.
                    at_ms: self.frames_total * 10 + 15,
                    scores: self.scores(&raw),
                });
            }
        }
        Ok(out)
    }

    fn reset(&mut self) {
        self.stream.reset();
        self.window.clear();
        let silence = vec![self.silence; self.spec.n_mels];
        self.window
            .extend(std::iter::repeat_n(silence, self.spec.frames));
        self.since_step = 0;
        self.frames_total = 0;
    }
}
