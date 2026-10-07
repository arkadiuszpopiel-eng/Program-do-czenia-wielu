//! Oś czasu referencji AEC: próbki tego, co zagrało (48 kHz), indeksowane czasem odtworzenia.

use std::collections::VecDeque;

use voice_audio_contract::{Frame, Resampler};

/// Częstotliwość wewnętrzna DSP.
pub const DSP_RATE: u32 = 48_000;
/// Ile historii referencji trzymać (2 s).
const KEEP: usize = DSP_RATE as usize * 2;

/// Referencja na osi czasu urządzenia.
#[derive(Debug, Default)]
pub struct ReferenceTimeline {
    start: u64,
    buf: VecDeque<f32>,
    resampler: Option<Resampler>,
    next_index: Option<u64>,
}

impl ReferenceTimeline {
    /// Dopisuje ramkę (mono lub stereo, dowolna obsługiwana częstotliwość).
    pub fn push(&mut self, frame: &Frame) {
        let mono = frame.to_mono();
        let rate = frame.format.sample_rate;
        let ts_index = frame.ts.to_samples(DSP_RATE);
        let samples = if rate == DSP_RATE {
            self.resampler = None;
            mono
        } else {
            let r = self
                .resampler
                .get_or_insert_with(|| Resampler::new(rate, DSP_RATE));
            if r.from_rate() != rate {
                *r = Resampler::new(rate, DSP_RATE);
            }
            let mut out = Vec::with_capacity(mono.len() * 3);
            r.process(&mono, &mut out);
            out
        };
        // Ciągłość: przy strumieniu resamplowanym indeks wynika z poprzedniej ramki, chyba że
        // znacznik czasu odskoczył o > 20 ms (nowy strumień).
        let index = match self.next_index {
            Some(n) if rate != DSP_RATE && n.abs_diff(ts_index) < u64::from(DSP_RATE / 50) => n,
            _ => ts_index,
        };
        self.write(index, &samples);
        self.next_index = Some(index + samples.len() as u64);
    }

    fn write(&mut self, index: u64, samples: &[f32]) {
        if self.buf.is_empty() {
            self.start = index;
        }
        let end = self.start + self.buf.len() as u64;
        if index < self.start {
            return; // starsze niż historia — pomijamy
        }
        if index > end + KEEP as u64 {
            self.buf.clear();
            self.start = index;
        } else if index > end {
            self.buf
                .extend(std::iter::repeat_n(0.0, (index - end) as usize));
        }
        let pos = (index - self.start) as usize;
        for (i, &s) in samples.iter().enumerate() {
            match self.buf.get_mut(pos + i) {
                Some(slot) => *slot = s,
                None => self.buf.push_back(s),
            }
        }
        while self.buf.len() > KEEP {
            self.buf.pop_front();
            self.start += 1;
        }
    }

    /// Kopiuje referencję od indeksu `index` do `out` (zera tam, gdzie brak danych).
    pub fn read(&self, index: i64, out: &mut [f32]) {
        for (i, o) in out.iter_mut().enumerate() {
            let at = index + i as i64;
            *o = if at < self.start as i64 {
                0.0
            } else {
                self.buf
                    .get((at - self.start as i64) as usize)
                    .copied()
                    .unwrap_or(0.0)
            };
        }
    }

    /// Czyści historię.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voice_audio_contract::MediaTime;

    #[test]
    fn indexes_by_play_time_and_fills_gaps() {
        let mut t = ReferenceTimeline::default();
        t.push(&Frame::mono(
            vec![1.0; 480],
            DSP_RATE,
            MediaTime::from_ms(1_000),
        ));
        t.push(&Frame::mono(
            vec![2.0; 480],
            DSP_RATE,
            MediaTime::from_ms(1_020),
        ));
        let mut out = vec![9.0; 1_440];
        t.read(48_000 - 480, &mut out);
        assert!(out[..480].iter().all(|&s| s == 0.0));
        assert!(out[480..960].iter().all(|&s| s == 1.0));
        assert!(out[960..1_440].iter().all(|&s| s == 0.0));
        let mut out = vec![0.0; 480];
        t.read(48_960, &mut out);
        assert!(out.iter().all(|&s| s == 2.0));
        t.read(10_000_000, &mut out);
        assert!(out.iter().all(|&s| s == 0.0));
        t.clear();
        t.read(48_000, &mut out);
        assert!(out.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn resampled_stream_is_contiguous() {
        let mut t = ReferenceTimeline::default();
        for i in 0..10 {
            t.push(&Frame::mono(
                vec![0.5; 240],
                24_000,
                MediaTime::from_ms(10 * i),
            ));
        }
        let mut out = vec![0.0; 4_000];
        t.read(200, &mut out);
        assert!(out.iter().all(|&s| (s - 0.5).abs() < 0.01));
    }
}
