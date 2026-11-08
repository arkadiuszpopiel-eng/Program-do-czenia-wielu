//! Czas urządzenia, format i ramka PCM.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AudioError;

/// Chwila na zegarze urządzenia audio w nanosekundach od dowolnego, stałego punktu
/// (Windows: QPC/`IAudioClock`; atrapa: zegar wirtualny). Monotoniczna w obrębie strumienia.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(transparent)]
pub struct MediaTime(pub u64);

impl MediaTime {
    /// Zero zegara.
    pub const ZERO: MediaTime = MediaTime(0);

    /// Z milisekund.
    pub const fn from_ms(ms: u64) -> Self {
        Self(ms * 1_000_000)
    }

    /// Z liczby próbek przy danej częstotliwości (zaokrąglenie w dół do ns).
    pub fn from_samples(samples: u64, sample_rate: u32) -> Self {
        let rate = u128::from(sample_rate.max(1));
        let ns = u128::from(samples) * 1_000_000_000 / rate;
        Self(u64::try_from(ns).unwrap_or(u64::MAX))
    }

    /// Nanosekundy.
    pub const fn as_nanos(self) -> u64 {
        self.0
    }

    /// Milisekundy (w dół).
    pub const fn as_ms(self) -> u64 {
        self.0 / 1_000_000
    }

    /// Liczba próbek odpowiadająca tej chwili (w dół).
    pub fn to_samples(self, sample_rate: u32) -> u64 {
        let v = u128::from(self.0) * u128::from(sample_rate) / 1_000_000_000;
        u64::try_from(v).unwrap_or(u64::MAX)
    }

    /// Przesunięcie o czas trwania (nasycające).
    #[must_use]
    pub fn plus(self, d: Duration) -> Self {
        let ns = u64::try_from(d.as_nanos()).unwrap_or(u64::MAX);
        Self(self.0.saturating_add(ns))
    }

    /// Cofnięcie o czas trwania (nasycające do zera).
    #[must_use]
    pub fn minus(self, d: Duration) -> Self {
        let ns = u64::try_from(d.as_nanos()).unwrap_or(u64::MAX);
        Self(self.0.saturating_sub(ns))
    }

    /// Odstęp od wcześniejszej chwili (zero, gdy `earlier` jest późniejsza).
    pub fn since(self, earlier: MediaTime) -> Duration {
        Duration::from_nanos(self.0.saturating_sub(earlier.0))
    }
}

impl fmt::Display for MediaTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{:03} ms",
            self.0 / 1_000_000,
            (self.0 / 1_000) % 1_000
        )
    }
}

/// Zegar mediów (urządzenie albo wirtualny).
pub trait MediaClock: Send + Sync {
    /// Bieżąca chwila.
    fn now(&self) -> MediaTime;
}

/// Ręczny zegar wirtualny: czas płynie wyłącznie przez [`ManualMediaClock::advance`].
#[derive(Debug, Default)]
pub struct ManualMediaClock(AtomicU64);

impl ManualMediaClock {
    /// Zegar od zera.
    pub fn new() -> Self {
        Self::default()
    }

    /// Przesuwa zegar.
    pub fn advance(&self, d: Duration) -> MediaTime {
        let ns = u64::try_from(d.as_nanos()).unwrap_or(u64::MAX);
        MediaTime(self.0.fetch_add(ns, Ordering::SeqCst).saturating_add(ns))
    }

    /// Ustawia zegar (nie cofa: przyjmuje maksimum).
    pub fn set(&self, t: MediaTime) {
        self.0.fetch_max(t.0, Ordering::SeqCst);
    }
}

impl MediaClock for ManualMediaClock {
    fn now(&self) -> MediaTime {
        MediaTime(self.0.load(Ordering::SeqCst))
    }
}

/// Częstotliwości próbkowania akceptowane w ramkach potoku (urządzenia, TTS, STT).
pub const SUPPORTED_RATES: [u32; 6] = [8_000, 16_000, 22_050, 24_000, 44_100, 48_000];

/// Wewnętrzna częstotliwość potoku rozpoznawania (DSP → VAD → STT).
pub const PIPELINE_RATE: u32 = 16_000;

/// Format ramki: częstotliwość i liczba kanałów (próbki `f32`, przeplatane).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct AudioFormat {
    /// Częstotliwość próbkowania (Hz), jedna z [`SUPPORTED_RATES`].
    pub sample_rate: u32,
    /// Liczba kanałów: 1 (mono) albo 2 (stereo).
    pub channels: u16,
}

impl AudioFormat {
    /// Mono.
    pub const fn mono(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            channels: 1,
        }
    }

    /// Stereo.
    pub const fn stereo(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            channels: 2,
        }
    }

    /// Sprawdza częstotliwość i liczbę kanałów.
    pub fn validate(&self) -> Result<(), AudioError> {
        if !SUPPORTED_RATES.contains(&self.sample_rate) {
            return Err(AudioError::Format(format!(
                "nieobsługiwana częstotliwość {} Hz",
                self.sample_rate
            )));
        }
        if !(1..=2).contains(&self.channels) {
            return Err(AudioError::Format(format!(
                "nieobsługiwana liczba kanałów {}",
                self.channels
            )));
        }
        Ok(())
    }

    /// Liczba próbek (wszystkich kanałów) na podany czas.
    pub fn samples_for(&self, d: Duration) -> usize {
        let frames =
            MediaTime(u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)).to_samples(self.sample_rate);
        usize::try_from(frames).unwrap_or(usize::MAX) * usize::from(self.channels)
    }
}

/// Ramka PCM (zwykle 10–20 ms) ze znacznikiem czasu pierwszej próbki na zegarze urządzenia.
/// Próbki `f32` w zakresie [-1, 1], kanały przeplatane. Ramka jest niezmienna i tania w klonowaniu.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Próbki (przeplatane).
    pub pcm: Arc<[f32]>,
    /// Format.
    pub format: AudioFormat,
    /// Czas pierwszej próbki (wejście: przechwycenie; wyjście/referencja: odtworzenie na urządzeniu).
    pub ts: MediaTime,
}

impl Frame {
    /// Nowa ramka z walidacją formatu i długości (wielokrotność liczby kanałów).
    pub fn new(
        pcm: impl Into<Arc<[f32]>>,
        format: AudioFormat,
        ts: MediaTime,
    ) -> Result<Self, AudioError> {
        format.validate()?;
        let pcm = pcm.into();
        if pcm.len() % usize::from(format.channels) != 0 {
            return Err(AudioError::Format(format!(
                "długość {} nie jest wielokrotnością liczby kanałów {}",
                pcm.len(),
                format.channels
            )));
        }
        Ok(Self { pcm, format, ts })
    }

    /// Ramka mono bez walidacji częstotliwości (dla wewnętrznych potoków o znanym formacie).
    pub fn mono(pcm: impl Into<Arc<[f32]>>, sample_rate: u32, ts: MediaTime) -> Self {
        Self {
            pcm: pcm.into(),
            format: AudioFormat::mono(sample_rate),
            ts,
        }
    }

    /// Liczba próbek na kanał.
    pub fn frames(&self) -> usize {
        self.pcm.len() / usize::from(self.format.channels.max(1))
    }

    /// Czas trwania.
    pub fn duration(&self) -> Duration {
        Duration::from_nanos(
            MediaTime::from_samples(self.frames() as u64, self.format.sample_rate).as_nanos(),
        )
    }

    /// Chwila tuż po ostatniej próbce.
    pub fn end_ts(&self) -> MediaTime {
        self.ts.plus(self.duration())
    }

    /// Próbki zmiksowane do mono (średnia kanałów).
    pub fn to_mono(&self) -> Vec<f32> {
        downmix(&self.pcm, self.format.channels)
    }
}

/// Miksuje próbki przeplatane do mono (średnia kanałów).
pub fn downmix(pcm: &[f32], channels: u16) -> Vec<f32> {
    let ch = usize::from(channels.max(1));
    if ch == 1 {
        return pcm.to_vec();
    }
    let scale = 1.0 / ch as f32;
    pcm.chunks_exact(ch)
        .map(|f| f.iter().sum::<f32>() * scale)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_time_conversions() {
        let t = MediaTime::from_samples(480, 48_000);
        assert_eq!(t.as_ms(), 10);
        assert_eq!(t.to_samples(16_000), 160);
        assert_eq!(
            MediaTime::from_ms(20)
                .plus(Duration::from_millis(5))
                .as_ms(),
            25
        );
        assert_eq!(
            MediaTime::from_ms(5).minus(Duration::from_millis(9)),
            MediaTime::ZERO
        );
        assert_eq!(
            MediaTime::from_ms(30).since(MediaTime::from_ms(10)),
            Duration::from_millis(20)
        );
        assert_eq!(MediaTime::from_ms(1).to_string(), "1.000 ms");
    }

    #[test]
    fn manual_clock_is_monotonic() {
        let c = ManualMediaClock::new();
        c.advance(Duration::from_millis(10));
        c.set(MediaTime::from_ms(5));
        assert_eq!(c.now().as_ms(), 10);
        c.set(MediaTime::from_ms(40));
        assert_eq!(c.now().as_ms(), 40);
    }

    #[test]
    fn frame_validation_and_helpers() {
        let f = Frame::new(
            vec![0.5, -0.5, 1.0, 0.0],
            AudioFormat::stereo(48_000),
            MediaTime::ZERO,
        )
        .unwrap();
        assert_eq!(f.frames(), 2);
        assert_eq!(f.to_mono(), vec![0.0, 0.5]);
        assert!(Frame::new(vec![0.0; 3], AudioFormat::stereo(48_000), MediaTime::ZERO).is_err());
        assert!(Frame::new(vec![0.0; 2], AudioFormat::mono(12_345), MediaTime::ZERO).is_err());
        assert!(
            AudioFormat {
                sample_rate: 16_000,
                channels: 3
            }
            .validate()
            .is_err()
        );
        let m = Frame::mono(vec![0.0; 160], 16_000, MediaTime::from_ms(100));
        assert_eq!(m.duration(), Duration::from_millis(10));
        assert_eq!(m.end_ts().as_ms(), 110);
        assert_eq!(
            AudioFormat::stereo(48_000).samples_for(Duration::from_millis(10)),
            960
        );
    }
}
