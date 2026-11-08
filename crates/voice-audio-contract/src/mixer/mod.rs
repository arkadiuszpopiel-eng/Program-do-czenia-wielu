//! Mikser wyjścia: tor głosu (jedna wypowiedź naraz, licznik wyrenderowanych próbek) + tor efektów,
//! ducking z rampą, wygaszane zatrzymanie, odczep referencji dla AEC.
//!
//! Podział: [`MixerControl`] (wątek sterujący — wolno alokować) ↔ [`MixerRender`] (wątek RT —
//! zero alokacji, zero blokad). Komunikacja wyłącznie przez kolejki SPSC `rtrb` i atomiki.
//! Ta sama logika działa w `-impl` (WASAPI) i `-fake` (wirtualne urządzenie).

mod control;
mod render;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

pub use control::{MixerControl, MixerReport, MixerStats};
pub use render::MixerRender;

use crate::frame::MediaTime;
use crate::types::Lane;

/// Konfiguracja miksera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixerConfig {
    /// Częstotliwość urządzenia.
    pub sample_rate: u32,
    /// Pojemność toru głosu (ms).
    pub voice_capacity_ms: u32,
    /// Pojemność toru efektów (ms).
    pub effects_capacity_ms: u32,
    /// Czy zbierać referencję (to, co gra) dla AEC.
    pub reference: bool,
}

impl MixerConfig {
    /// Domyślnie: 30 s głosu (≈ 5,8 MB przy 48 kHz), 5 s efektów, referencja włączona.
    pub const fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            voice_capacity_ms: 30_000,
            effects_capacity_ms: 5_000,
            reference: true,
        }
    }

    fn samples(&self, ms: u32) -> usize {
        (u64::from(self.sample_rate) * u64::from(ms) / 1000).max(1) as usize
    }
}

/// Polecenie sterowania → RT (kopiowalne, bez alokacji).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Cmd {
    Chunk {
        lane: Lane,
        utterance: u64,
        len: u32,
    },
    End {
        lane: Lane,
        utterance: u64,
    },
    Duck {
        target: f32,
        ramp: u32,
    },
    Stop {
        lane: Lane,
        fade: u32,
    },
    Gain {
        lane: Lane,
        gain: f32,
    },
}

/// Raport RT → sterowanie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Report {
    Started {
        lane: Lane,
        utterance: u64,
    },
    Finished {
        lane: Lane,
        utterance: u64,
        rendered: u64,
        stopped: bool,
    },
    Underrun {
        lane: Lane,
    },
    Overflow {
        lane: Lane,
    },
}

/// Blok referencji: `len` próbek mono odtworzonych od chwili `ts`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RefBlock {
    pub ts: MediaTime,
    pub len: u32,
}

/// Postęp bieżącej wypowiedzi toru głosu (seqlock: jeden pisarz RT, bez blokowania pisarza).
#[derive(Debug, Default)]
pub(crate) struct Progress {
    seq: AtomicU32,
    active: AtomicBool,
    utterance: AtomicU64,
    rendered: AtomicU64,
}

impl Progress {
    pub(crate) fn publish(&self, current: Option<u64>, rendered: u64) {
        self.seq.fetch_add(1, Ordering::AcqRel);
        self.active.store(current.is_some(), Ordering::SeqCst);
        self.utterance.store(current.unwrap_or(0), Ordering::SeqCst);
        self.rendered.store(rendered, Ordering::SeqCst);
        self.seq.fetch_add(1, Ordering::AcqRel);
    }

    /// Spójny odczyt (ponawiany, gdy pisarz jest w trakcie zapisu).
    pub(crate) fn read(&self) -> Option<(u64, u64)> {
        for _ in 0..1_000 {
            let s1 = self.seq.load(Ordering::SeqCst);
            if s1 % 2 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let active = self.active.load(Ordering::SeqCst);
            let utterance = self.utterance.load(Ordering::SeqCst);
            let rendered = self.rendered.load(Ordering::SeqCst);
            if self.seq.load(Ordering::SeqCst) == s1 {
                return active.then_some((utterance, rendered));
            }
        }
        None
    }
}

/// Stan współdzielony RT ↔ sterowanie (tylko atomiki).
#[derive(Debug, Default)]
pub(crate) struct Shared {
    pub rendered: AtomicU64,
    pub underruns: AtomicU64,
    pub dropped_reports: AtomicU64,
    pub dropped_reference: AtomicU64,
    pub duck_gain_bits: AtomicU32,
    pub progress: Progress,
}

/// Tworzy parę sterowanie/RT.
pub fn mixer(config: MixerConfig) -> (MixerControl, MixerRender) {
    let (cmd_tx, cmd_rx) = rtrb::RingBuffer::new(8_192);
    let (rep_tx, rep_rx) = rtrb::RingBuffer::new(8_192);
    let (voice_tx, voice_rx) = rtrb::RingBuffer::new(config.samples(config.voice_capacity_ms));
    let (fx_tx, fx_rx) = rtrb::RingBuffer::new(config.samples(config.effects_capacity_ms));
    let shared = Arc::new(Shared {
        duck_gain_bits: AtomicU32::new(1.0f32.to_bits()),
        ..Shared::default()
    });
    let (ref_rt, ref_ctl) = if config.reference {
        let (s_tx, s_rx) = rtrb::RingBuffer::new(config.samples(2_000));
        let (b_tx, b_rx) = rtrb::RingBuffer::new(1_024);
        (Some((s_tx, b_tx)), Some((s_rx, b_rx)))
    } else {
        (None, None)
    };
    let render = MixerRender::new(cmd_rx, rep_tx, voice_rx, fx_rx, ref_rt, Arc::clone(&shared));
    let control = MixerControl::new(config, cmd_tx, rep_rx, voice_tx, fx_tx, ref_ctl, shared);
    (control, render)
}

#[cfg(test)]
mod tests;
