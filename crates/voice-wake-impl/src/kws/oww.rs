//! Potok openWakeWord (Apache-2.0; modele mel i embedding z wydania openWakeWord, klasyfikatory —
//! własny trening frazy PL): co 80 ms (1280 próbek + 480 próbek zakładki) `melspectrogram.onnx`
//! → ostatnie 8 ramek `x/10 + 2` → bufor 76 ramek → `embedding_model.onnx` (96 cech) → bufor
//! 16 embeddingów → klasyfikator per fraza (`[1, 16, 96]` → prawdopodobieństwo).
//! Na starcie segmentu bufory jak w openWakeWord: ramki mel = 1, embeddingi z takiego okna.

use std::collections::VecDeque;

use voice_wake_contract::{KeywordScorer, KwsScores, WakeError};

use super::onnx::{Plan, err};

const CHUNK: usize = 1_280;
const OVERLAP: usize = 480;
const MELS: usize = 32;
const MEL_WINDOW: usize = 76;
const MEL_STEP: usize = 8;
const EMB: usize = 96;
const EMB_WINDOW: usize = 16;

/// Model `openwakeword`.
pub(crate) struct OwwScorer {
    labels: Vec<String>,
    mel: Plan,
    emb: Plan,
    classifiers: Vec<Plan>,
    pending: Vec<f32>,
    history: Vec<f32>,
    mel_buf: VecDeque<Vec<f32>>,
    emb_buf: VecDeque<Vec<f32>>,
    emb_init: Vec<f32>,
    t_ms: u64,
}

impl OwwScorer {
    pub(crate) fn new(
        labels: Vec<String>,
        mel: &[u8],
        emb: &[u8],
        classifiers: &[Vec<u8>],
    ) -> Result<Self, WakeError> {
        let mel = Plan::load(mel, &[1, CHUNK + OVERLAP])?;
        let emb = Plan::load(emb, &[1, MEL_WINDOW, MELS, 1])?;
        let classifiers = classifiers
            .iter()
            .map(|b| Plan::load(b, &[1, EMB_WINDOW, EMB]))
            .collect::<Result<Vec<_>, _>>()?;
        let emb_init = emb.run(&vec![1.0; MEL_WINDOW * MELS])?;
        if emb_init.len() != EMB {
            return Err(err(format!("embedding: {} cech ≠ {EMB}", emb_init.len())));
        }
        let mut s = Self {
            labels,
            mel,
            emb,
            classifiers,
            pending: Vec::with_capacity(CHUNK),
            history: Vec::new(),
            mel_buf: VecDeque::new(),
            emb_buf: VecDeque::new(),
            emb_init,
            t_ms: 0,
        };
        s.reset();
        Ok(s)
    }

    fn step(&mut self, chunk: &[f32]) -> Result<KwsScores, WakeError> {
        // openWakeWord podaje próbki w skali int16.
        let input: Vec<f32> = self
            .history
            .iter()
            .chain(chunk)
            .map(|x| x * 32_768.0)
            .collect();
        self.history = chunk[CHUNK - OVERLAP..].to_vec();
        let mel = self.mel.run(&input)?;
        let frames = mel.len() / MELS;
        if frames < MEL_STEP {
            return Err(err(format!("melspectrogram: {frames} ramek < {MEL_STEP}")));
        }
        for f in frames - MEL_STEP..frames {
            let row = mel[f * MELS..(f + 1) * MELS]
                .iter()
                .map(|x| x / 10.0 + 2.0)
                .collect();
            self.mel_buf.push_back(row);
            if self.mel_buf.len() > MEL_WINDOW {
                self.mel_buf.pop_front();
            }
        }
        let window: Vec<f32> = self.mel_buf.iter().flatten().copied().collect();
        let e = self.emb.run(&window)?;
        self.emb_buf.push_back(e);
        if self.emb_buf.len() > EMB_WINDOW {
            self.emb_buf.pop_front();
        }
        let feats: Vec<f32> = self.emb_buf.iter().flatten().copied().collect();
        let scores = self
            .classifiers
            .iter()
            .map(|c| {
                c.run(&feats)
                    .map(|o| o.first().copied().unwrap_or(0.0).clamp(0.0, 1.0))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.t_ms += 80;
        Ok(KwsScores {
            at_ms: self.t_ms,
            scores,
        })
    }
}

impl KeywordScorer for OwwScorer {
    fn labels(&self) -> &[String] {
        &self.labels
    }

    fn push(&mut self, samples: &[f32]) -> Result<Vec<KwsScores>, WakeError> {
        self.pending.extend_from_slice(samples);
        let mut out = Vec::new();
        while self.pending.len() >= CHUNK {
            let chunk: Vec<f32> = self.pending.drain(..CHUNK).collect();
            out.push(self.step(&chunk)?);
        }
        Ok(out)
    }

    fn reset(&mut self) {
        self.pending.clear();
        self.history = vec![0.0; OVERLAP];
        self.mel_buf = std::iter::repeat_n(vec![1.0; MELS], MEL_WINDOW).collect();
        self.emb_buf = std::iter::repeat_n(self.emb_init.clone(), EMB_WINDOW).collect();
        self.t_ms = 0;
    }
}
