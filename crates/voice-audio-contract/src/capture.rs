//! Kolejka przechwytywania: wątek RT urządzenia wejściowego → konsument ramek.
//! Pisarz ([`CaptureWriter`]) nie alokuje i nie blokuje; czytelnik ([`CaptureReader`]) składa
//! ramki `frame_ms` ze znacznikami czasu z zegara urządzenia (ekstrapolacja od ostatniego znacznika).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rtrb::{Consumer, Producer};

use crate::InputStream;
use crate::frame::{AudioFormat, Frame, MediaTime};

#[derive(Debug, Clone, Copy)]
struct Mark {
    index: u64,
    ts: MediaTime,
}

#[derive(Debug, Default)]
struct CaptureShared {
    overruns: AtomicU64,
    latency_ns: AtomicU64,
}

/// Strona RT (wątek urządzenia).
#[derive(Debug)]
pub struct CaptureWriter {
    samples: Producer<f32>,
    marks: Producer<Mark>,
    channels: usize,
    written: u64,
    shared: Arc<CaptureShared>,
}

impl CaptureWriter {
    /// Zapisuje blok próbek przeplatanych przechwyconych w chwili `ts` (pierwsza próbka).
    /// Gdy konsument nie nadąża, cały blok jest odrzucany (licznik `overruns`) — bez blokowania.
    pub fn write(&mut self, interleaved: &[f32], ts: MediaTime) {
        let frames = interleaved.len() / self.channels;
        let n = frames * self.channels;
        if self.samples.slots() < n {
            self.shared.overruns.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let _ = self.marks.push(Mark {
            index: self.written,
            ts,
        });
        let _ = self.samples.push_entire_slice(&interleaved[..n]);
        self.written += frames as u64;
    }

    /// Czy czytelnik został zamknięty (strumień wejściowy porzucony przez konsumenta) — wątek
    /// urządzenia może przestać przechwytywać.
    pub fn is_abandoned(&self) -> bool {
        self.samples.is_abandoned()
    }

    /// Aktualizuje opóźnienie wejścia (np. z rozmiaru bufora urządzenia).
    pub fn set_latency(&self, latency: Duration) {
        self.shared.latency_ns.store(
            u64::try_from(latency.as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    }
}

/// Strona konsumenta: ramki o stałej długości.
#[derive(Debug)]
pub struct CaptureReader {
    samples: Consumer<f32>,
    marks: Consumer<Mark>,
    format: AudioFormat,
    frame_samples: usize,
    read_frames: u64,
    last_mark: Option<Mark>,
    shared: Arc<CaptureShared>,
}

/// Tworzy parę pisarz/czytelnik: ramki `frame_ms`, kolejka `queue_ms`.
pub fn capture_ring(
    format: AudioFormat,
    frame_ms: u32,
    queue_ms: u32,
) -> (CaptureWriter, CaptureReader) {
    let ch = usize::from(format.channels.max(1));
    let per_ms = format.sample_rate as usize * ch / 1000;
    let capacity = (per_ms * queue_ms.max(frame_ms * 2) as usize).max(ch);
    let (s_tx, s_rx) = rtrb::RingBuffer::new(capacity);
    let (m_tx, m_rx) = rtrb::RingBuffer::new(1_024);
    let shared = Arc::new(CaptureShared::default());
    let frame_samples = (format.sample_rate as usize * frame_ms as usize / 1000).max(1) * ch;
    (
        CaptureWriter {
            samples: s_tx,
            marks: m_tx,
            channels: ch,
            written: 0,
            shared: Arc::clone(&shared),
        },
        CaptureReader {
            samples: s_rx,
            marks: m_rx,
            format,
            frame_samples,
            read_frames: 0,
            last_mark: None,
            shared,
        },
    )
}

impl CaptureReader {
    fn ts_for(&mut self, index: u64) -> MediaTime {
        while let Ok(m) = self.marks.peek().copied() {
            if m.index <= index {
                self.last_mark = Some(m);
                let _ = self.marks.pop();
            } else {
                break;
            }
        }
        match self.last_mark {
            Some(m) => m.ts.plus(Duration::from_nanos(
                MediaTime::from_samples(index - m.index, self.format.sample_rate).0,
            )),
            None => MediaTime::from_samples(index, self.format.sample_rate),
        }
    }
}

impl InputStream for CaptureReader {
    fn format(&self) -> AudioFormat {
        self.format
    }

    fn read(&mut self) -> Option<Frame> {
        if self.samples.slots() < self.frame_samples {
            return None;
        }
        let mut pcm = vec![0.0f32; self.frame_samples];
        self.samples.pop_entire_slice(&mut pcm).ok()?;
        let ts = self.ts_for(self.read_frames);
        self.read_frames += (self.frame_samples / usize::from(self.format.channels.max(1))) as u64;
        Some(Frame {
            pcm: pcm.into(),
            format: self.format,
            ts,
        })
    }

    fn overruns(&self) -> u64 {
        self.shared.overruns.load(Ordering::Relaxed)
    }

    fn latency(&self) -> Duration {
        Duration::from_nanos(self.shared.latency_ns.load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_assembled_with_extrapolated_timestamps() {
        let (mut w, mut r) = capture_ring(AudioFormat::mono(16_000), 10, 200);
        assert!(r.read().is_none());
        w.write(&[0.1; 240], MediaTime::from_ms(1_000));
        w.write(&[0.2; 240], MediaTime::from_ms(1_015));
        let a = r.read().unwrap();
        let b = r.read().unwrap();
        let c = r.read().unwrap();
        assert!(r.read().is_none());
        assert_eq!(a.pcm.len(), 160);
        assert_eq!(a.ts.as_ms(), 1_000);
        assert_eq!(b.ts.as_ms(), 1_010);
        assert_eq!(c.ts.as_ms(), 1_020);
        assert!((b.pcm[79] - 0.1).abs() < 1e-6 && (b.pcm[80] - 0.2).abs() < 1e-6);
        w.set_latency(Duration::from_millis(12));
        assert_eq!(r.latency(), Duration::from_millis(12));
        assert_eq!(r.format(), AudioFormat::mono(16_000));
    }

    #[test]
    fn overrun_drops_whole_block_and_counts() {
        let (mut w, mut r) = capture_ring(AudioFormat::stereo(8_000), 10, 20);
        for i in 0..10 {
            w.write(&[0.5; 160], MediaTime::from_ms(i * 10));
        }
        assert!(r.overruns() > 0);
        let mut n = 0;
        while let Some(f) = r.read() {
            assert_eq!(f.frames(), 80);
            n += 1;
        }
        assert!(n >= 2);
    }
}
