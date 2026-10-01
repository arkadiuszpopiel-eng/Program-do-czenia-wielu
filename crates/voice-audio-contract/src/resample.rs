//! Resampler strumieniowy: okienkowany sinc (okno Kaisera), tablica faz z interpolacją liniową,
//! dowolny stosunek częstotliwości (także ułamkowy — zmiana wysokości głosu w `voice-tts`).
//! Własna implementacja (bez zależności): pełna kontrola alokacji i licencji.

use std::f64::consts::PI;

/// Liczba przejść przez zero sinc po każdej stronie (w jednostkach niższej częstotliwości).
const ZERO_CROSSINGS: usize = 24;
/// Nadpróbkowanie tablicy jądra (faz na próbkę wejściową).
const TABLE_OVERSAMPLE: usize = 256;
/// Parametr okna Kaisera (≈ 80 dB tłumienia bocznego).
const KAISER_BETA: f64 = 8.0;
/// Pasmo przepustowe względem Nyquista niższej częstotliwości.
const CUTOFF: f64 = 0.92;

/// Resampler mono ze stanem (ciągłość między fragmentami).
#[derive(Debug, Clone)]
pub struct Resampler {
    from: u32,
    to: u32,
    step: f64,
    half: usize,
    fc: f64,
    table: Vec<f32>,
    buf: Vec<f32>,
    t: f64,
    in_total: u64,
    out_total: u64,
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..50 {
        term *= q / (k as f64 * k as f64);
        sum += term;
        if term < sum * 1e-12 {
            break;
        }
    }
    sum
}

impl Resampler {
    /// Resampler `from` → `to` (Hz); `from == to` przepuszcza bez zmian.
    pub fn new(from: u32, to: u32) -> Self {
        Self::with_ratio(from, to, f64::from(to.max(1)) / f64::from(from.max(1)))
    }

    /// Resampler o dowolnym stosunku `ratio = wyjście / wejście` (np. 1/1,12 przy podnoszeniu tonu).
    pub fn with_ratio(from: u32, to: u32, ratio: f64) -> Self {
        let ratio = ratio.clamp(1e-3, 1e3);
        let fc = CUTOFF * ratio.min(1.0);
        let half = ((ZERO_CROSSINGS as f64) / ratio.min(1.0)).ceil() as usize;
        let len = 2 * half * TABLE_OVERSAMPLE + 1;
        let denom = bessel_i0(KAISER_BETA);
        let table = (0..len)
            .map(|i| {
                let x = i as f64 / TABLE_OVERSAMPLE as f64 - half as f64;
                let sinc = if x.abs() < 1e-12 {
                    1.0
                } else {
                    (PI * fc * x).sin() / (PI * fc * x)
                };
                let r = x / half as f64;
                let w = if r.abs() >= 1.0 {
                    0.0
                } else {
                    bessel_i0(KAISER_BETA * (1.0 - r * r).sqrt()) / denom
                };
                (fc * sinc * w) as f32
            })
            .collect();
        Self {
            from,
            to,
            step: 1.0 / ratio,
            half,
            fc,
            table,
            buf: vec![0.0; half],
            t: half as f64,
            in_total: 0,
            out_total: 0,
        }
    }

    /// Częstotliwość wejścia.
    pub fn from_rate(&self) -> u32 {
        self.from
    }

    /// Częstotliwość wyjścia.
    pub fn to_rate(&self) -> u32 {
        self.to
    }

    /// Czy to przejście bez zmian.
    pub fn is_passthrough(&self) -> bool {
        (self.step - 1.0).abs() < 1e-12
    }

    /// Opóźnienie grupowe w próbkach wejścia (wyjście pojawia się po tylu próbkach).
    pub fn latency_in(&self) -> usize {
        self.half
    }

    /// Pasmo przepustowe (Hz).
    pub fn passband_hz(&self) -> f32 {
        (self.fc * f64::from(self.from) / 2.0) as f32
    }

    fn kernel(&self, x: f64) -> f32 {
        let pos = (x + self.half as f64) * TABLE_OVERSAMPLE as f64;
        if pos < 0.0 {
            return 0.0;
        }
        let i = pos.floor() as usize;
        if i + 1 >= self.table.len() {
            return 0.0;
        }
        let frac = (pos - i as f64) as f32;
        self.table[i] + (self.table[i + 1] - self.table[i]) * frac
    }

    /// Przetwarza fragment; wynik dopisuje do `out`. Wyjście jest opóźnione o [`Self::latency_in`]
    /// próbek wejścia — resztę oddaje [`Self::flush`].
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.is_passthrough() {
            out.extend_from_slice(input);
            return;
        }
        self.buf.extend_from_slice(input);
        self.in_total += input.len() as u64;
        let half = self.half as isize;
        loop {
            let ti = self.t.floor() as isize;
            if ti + half >= self.buf.len() as isize {
                break;
            }
            let frac = self.t - ti as f64;
            let mut acc = 0.0f32;
            for k in (1 - half)..=half {
                let idx = (ti + k) as usize;
                acc += self.buf[idx] * self.kernel(k as f64 - frac);
            }
            out.push(acc);
            self.out_total += 1;
            self.t += self.step;
        }
        let keep_from = (self.t.floor() as usize).saturating_sub(self.half);
        if keep_from > 0 {
            self.buf.drain(..keep_from);
            self.t -= keep_from as f64;
        }
    }

    /// Opróżnia stan (koniec strumienia): dopełnia zerami i oddaje ogon.
    pub fn flush(&mut self, out: &mut Vec<f32>) {
        if self.is_passthrough() {
            return;
        }
        let wanted = (self.in_total as f64 / self.step).round() as u64;
        let before = out.len();
        let produced_before = self.out_total;
        let pad = vec![0.0; self.half + 1];
        self.buf.extend_from_slice(&pad);
        let in_total = self.in_total;
        self.process(&[], out);
        self.in_total = in_total;
        // Ogon odpowiada tylko rzeczywistym próbkom wejścia — przytnij nadmiar z dopełnienia.
        let allowed = wanted.saturating_sub(produced_before) as usize;
        out.truncate(before + allowed.min(out.len() - before));
        self.reset();
    }

    /// Zeruje stan (nowy strumień).
    pub fn reset(&mut self) {
        self.buf.clear();
        self.buf.resize(self.half, 0.0);
        self.t = self.half as f64;
        self.in_total = 0;
        self.out_total = 0;
    }

    /// Jednorazowa konwersja całego sygnału (bez opóźnienia na wyjściu).
    pub fn convert(from: u32, to: u32, input: &[f32]) -> Vec<f32> {
        Self::convert_ratio(
            from,
            to,
            f64::from(to.max(1)) / f64::from(from.max(1)),
            input,
        )
    }

    /// Jednorazowa konwersja o dowolnym stosunku (długość ≈ `len · ratio`).
    pub fn convert_ratio(from: u32, to: u32, ratio: f64, input: &[f32]) -> Vec<f32> {
        let mut r = Self::with_ratio(from, to, ratio);
        if r.is_passthrough() {
            return input.to_vec();
        }
        let mut out = Vec::with_capacity((input.len() as f64 * ratio) as usize + 8);
        r.process(input, &mut out);
        r.flush(&mut out);
        out
    }
}

#[cfg(test)]
mod tests;
