//! Nasłuch słów wywoławczych (v1): ramki 16 kHz mono → bufor pierścieniowy (~2 s, **jedyne
//! miejsce, gdzie audio leży przed wykryciem**) → bramka energii → model KWS ([`KeywordScorer`])
//! → detektor z histerezą → opcjonalna bramka właściciela → [`WakeWordTrigger`].
//!
//! Niezmienniki prywatności (testowane): audio opuszcza nasłuch wyłącznie w wyzwalaczu (po
//! wykryciu); bufor ma stałą pojemność i nie ma publicznego odczytu; `Debug` nie pokazuje próbek;
//! wstrzymanie (DND, wyciszenie, trwające słuchanie) czyści bufor i nie przyjmuje audio.
//! Model dostaje audio tylko przy otwartej bramce (plus krótki pre-roll z bufora).

use voice_vad_contract::EnergyDetector;

use crate::detector::{KwsScores, WakeWordDetector, WakeWordHit};
use crate::words::KwsParams;
use crate::{WakeError, WakeWordCfg};

/// Częstotliwość wejścia nasłuchu i modeli KWS.
pub const KWS_RATE: u32 = 16_000;
/// Ramka bramki (10 ms).
const FRAME: usize = 160;

/// Model słów wywoławczych (ONNX w `-impl`, skrypt w `-fake`).
pub trait KeywordScorer: Send {
    /// Etykiety wyjść (frazy, kolejność wyników).
    fn labels(&self) -> &[String];
    /// Dokłada próbki 16 kHz mono; zwraca wyniki kroków domkniętych przez te próbki
    /// (`at_ms` liczone od ostatniego [`KeywordScorer::reset`]).
    fn push(&mut self, samples: &[f32]) -> Result<Vec<KwsScores>, WakeError>;
    /// Nowy segment (po otwarciu bramki).
    fn reset(&mut self);
}

/// Bramka właściciela (`voice-speaker`): czy fraza wywoławcza to głos właściciela.
pub trait OwnerCheck: Send {
    /// `Some(true)` — właściciel; `Some(false)` — obcy; `None` — nie wiadomo (odrzucane).
    fn is_owner(&mut self, audio: &[f32]) -> Option<bool>;
}

/// Wybudzenie z audio frazy (zawartość bufora pierścieniowego w chwili wykrycia).
pub struct WakeWordTrigger {
    /// Wykrycie.
    pub hit: WakeWordHit,
    /// Audio 16 kHz mono (≤ `ring_ms`) — dopiero teraz może opuścić moduł (np. pre-roll STT).
    pub audio: Vec<f32>,
}

impl std::fmt::Debug for WakeWordTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WakeWordTrigger")
            .field("hit", &self.hit)
            .field("audio_samples", &self.audio.len())
            .finish()
    }
}

/// Liczniki nasłuchu (Diagnostyka; bez treści).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListenerStats {
    /// Ramki 10 ms przyjęte.
    pub frames: u64,
    /// Ramki podane modelowi (CPU: model działa tylko przy otwartej bramce).
    pub scored_frames: u64,
    /// Ramki odrzucone przy wstrzymaniu.
    pub suspended_frames: u64,
    /// Wykrycia przekazane dalej.
    pub triggers: u64,
    /// Wykrycia odrzucone przez bramkę właściciela.
    pub owner_rejected: u64,
}

/// Bufor pierścieniowy o stałej pojemności (bez publicznego odczytu).
struct AudioRing {
    buf: Vec<f32>,
    head: usize,
    len: usize,
}

impl AudioRing {
    fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0.0; capacity.max(1)],
            head: 0,
            len: 0,
        }
    }

    fn push(&mut self, samples: &[f32]) {
        let cap = self.buf.len();
        for s in samples {
            self.buf[self.head] = *s;
            self.head = (self.head + 1) % cap;
            self.len = (self.len + 1).min(cap);
        }
    }

    /// Ostatnie `n` próbek (chronologicznie).
    fn latest(&self, n: usize) -> Vec<f32> {
        let cap = self.buf.len();
        let n = n.min(self.len);
        (0..n)
            .map(|i| self.buf[(self.head + cap - n + i) % cap])
            .collect()
    }

    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|s| *s = 0.0);
        self.head = 0;
        self.len = 0;
    }
}

/// Nasłuch słów wywoławczych.
pub struct WakeWordListener {
    scorer: Box<dyn KeywordScorer>,
    detector: WakeWordDetector,
    params: KwsParams,
    ring: AudioRing,
    gate: EnergyDetector,
    gate_open: bool,
    hang_ms: u32,
    origin_ms: u64,
    now_ms: u64,
    partial: Vec<f32>,
    suspended: bool,
    owner_gate: bool,
    owner: Option<Box<dyn OwnerCheck>>,
    stats: ListenerStats,
}

impl std::fmt::Debug for WakeWordListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WakeWordListener")
            .field("now_ms", &self.now_ms)
            .field("gate_open", &self.gate_open)
            .field("suspended", &self.suspended)
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl WakeWordListener {
    /// Nasłuch dla konfiguracji fraz, strojenia i modelu. Bramka właściciela (`owner_gate`)
    /// wymaga [`WakeWordListener::with_owner_check`] — bez niej każde wykrycie jest odrzucane.
    pub fn new(
        cfg: &WakeWordCfg,
        params: KwsParams,
        scorer: Box<dyn KeywordScorer>,
    ) -> Result<Self, WakeError> {
        let detector = WakeWordDetector::new(cfg, scorer.labels(), params)?;
        let ring = (KWS_RATE as usize * params.ring_ms as usize) / 1000;
        Ok(Self {
            scorer,
            detector,
            params,
            ring: AudioRing::new(ring),
            gate: EnergyDetector::new(),
            gate_open: !params.vad_gate,
            hang_ms: 0,
            origin_ms: 0,
            now_ms: 0,
            partial: Vec::with_capacity(FRAME),
            suspended: false,
            owner_gate: cfg.owner_gate,
            owner: None,
            stats: ListenerStats::default(),
        })
    }

    /// Bramka właściciela (`voice-speaker`).
    #[must_use]
    pub fn with_owner_check(mut self, owner: Box<dyn OwnerCheck>) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Zwraca model (np. runner FAR/FRR używa jednego modelu dla wielu nagrań).
    pub fn into_scorer(self) -> Box<dyn KeywordScorer> {
        self.scorer
    }

    /// Liczniki.
    pub fn stats(&self) -> ListenerStats {
        self.stats
    }

    /// Próbki przechowywane teraz (≤ pojemność bufora).
    pub fn retained_samples(&self) -> usize {
        self.ring.len + self.partial.len()
    }

    /// Pojemność bufora (próbki).
    pub fn capacity_samples(&self) -> usize {
        self.ring.buf.len() + FRAME
    }

    /// Czas strumienia (ms).
    pub fn now_ms(&self) -> u64 {
        self.now_ms
    }

    /// Wstrzymanie (DND, wyciszenie, trwające słuchanie): czyści bufor, nie przyjmuje audio.
    pub fn set_suspended(&mut self, on: bool) {
        if on && !self.suspended {
            self.ring.clear();
            self.partial.clear();
            self.scorer.reset();
            self.gate_open = !self.params.vad_gate;
            self.origin_ms = self.now_ms;
        }
        self.suspended = on;
    }

    /// Czy wstrzymany.
    pub fn is_suspended(&self) -> bool {
        self.suspended
    }

    /// Dokłada audio 16 kHz mono; zwraca wybudzenie (po pozytywnej bramce właściciela).
    pub fn push(&mut self, samples: &[f32]) -> Result<Option<WakeWordTrigger>, WakeError> {
        for scores in self.push_scores(samples)? {
            let Some(hit) = self.detector.push(&scores) else {
                continue;
            };
            let audio = self.ring.latest(self.ring.len);
            if self.owner_gate {
                let owner = self.owner.as_mut().and_then(|o| o.is_owner(&audio));
                if owner != Some(true) {
                    self.stats.owner_rejected += 1;
                    continue;
                }
            }
            self.stats.triggers += 1;
            self.ring.clear();
            self.scorer.reset();
            self.origin_ms = self.now_ms;
            return Ok(Some(WakeWordTrigger { hit, audio }));
        }
        Ok(None)
    }

    /// Tryb pomiarowy (runner FAR/FRR): bramka + model, wyniki bez detekcji. Audio nie wychodzi.
    pub fn push_scores(&mut self, samples: &[f32]) -> Result<Vec<KwsScores>, WakeError> {
        let mut out = Vec::new();
        if self.suspended {
            self.stats.suspended_frames += (samples.len() / FRAME) as u64;
            return Ok(out);
        }
        for s in samples {
            self.partial.push(*s);
            if self.partial.len() == FRAME {
                let frame = std::mem::take(&mut self.partial);
                self.frame(&frame, &mut out)?;
                self.partial = frame;
                self.partial.clear();
            }
        }
        Ok(out)
    }

    fn frame(&mut self, frame: &[f32], out: &mut Vec<KwsScores>) -> Result<(), WakeError> {
        self.stats.frames += 1;
        self.ring.push(frame);
        self.now_ms += 10;
        let was_open = self.gate_open;
        if self.params.vad_gate {
            if self.gate.prob(frame) >= 0.5 {
                self.hang_ms = self.params.gate_hang_ms;
                self.gate_open = true;
            } else if self.hang_ms >= 10 {
                self.hang_ms -= 10;
            } else {
                self.gate_open = false;
            }
        }
        let feed = if self.gate_open && !was_open {
            self.scorer.reset();
            let pre = (KWS_RATE as usize * self.params.gate_preroll_ms as usize) / 1000;
            let audio = self.ring.latest(pre.max(FRAME));
            self.origin_ms = self
                .now_ms
                .saturating_sub((audio.len() / FRAME) as u64 * 10);
            Some(audio)
        } else if self.gate_open {
            Some(frame.to_vec())
        } else {
            None
        };
        if let Some(audio) = feed {
            self.stats.scored_frames += (audio.len() / FRAME) as u64;
            for mut s in self.scorer.push(&audio)? {
                s.at_ms += self.origin_ms;
                out.push(s);
            }
        }
        Ok(())
    }
}
