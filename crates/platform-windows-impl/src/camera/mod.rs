//! `CameraPort` dla Windows: kamery USB i wbudowane przez Media Foundation (`mf.rs`, `win.rs`).
//!
//! Każda otwarta kamera ma własny wątek roboczy (COM MTA + `MFStartup`), który trzyma czytnik
//! źródła i w pętli czyta klatki RGB32; ostatnia klatka leży w pamięci współdzielonej, a
//! `CameraStream::read` czeka na klatkę nowszą od poprzednio wydanej. Dzięki temu strumień jest
//! `Send`, odczyt ma limit czasu, a klatka jest bieżąca (nie z kolejki sprzed sekund). Nic nie
//! trafia na dysk. Poza Windows port zwraca `Unsupported`.

#[cfg(windows)]
mod mf;
#[cfg(windows)]
mod win;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use platform_contract::{CameraDevice, CameraPort, CameraStream, PlatformError, RgbaImage};

/// Ile czekamy na zakończenie wątku kamery przy zamykaniu strumienia.
const CLOSE_WAIT: Duration = Duration::from_secs(3);

/// Kamery systemu Windows (bezstanowe; każde otwarcie tworzy własny wątek).
#[derive(Debug, Default, Clone, Copy)]
pub struct WinCameras;

impl CameraPort for WinCameras {
    fn devices(&self) -> Result<Vec<CameraDevice>, PlatformError> {
        #[cfg(windows)]
        {
            mf::devices()
        }
        #[cfg(not(windows))]
        {
            Err(unsupported())
        }
    }

    fn open(&self, id: &str, max_side: u32) -> Result<Box<dyn CameraStream>, PlatformError> {
        #[cfg(windows)]
        {
            win::open(id, max_side)
        }
        #[cfg(not(windows))]
        {
            let _ = (id, max_side);
            Err(unsupported())
        }
    }
}

#[cfg(not(windows))]
fn unsupported() -> PlatformError {
    PlatformError::Unsupported("kamery: Media Foundation działa tylko na Windows".into())
}

/// Tryb natywny kamery (jeden wpis listy typów strumienia wideo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Mode {
    pub width: u32,
    pub height: u32,
    /// Klatki na sekundę (zaokrąglone w dół; 0 = nieznane).
    pub fps: u32,
    /// Format skompresowany (MJPG, H264) — wymaga dekodera, więc przegrywa remisy.
    pub compressed: bool,
}

/// Wybiera tryb najbliższy `max_side`: najmniejszy, którego dłuższy bok jest ≥ `max_side`,
/// a gdy takiego nie ma — największy. Remisy: płynny (≥ 15 kl./s), nieskompresowany, szybszy.
pub(crate) fn pick_mode(modes: &[Mode], max_side: u32) -> Option<usize> {
    let side = |m: &Mode| m.width.max(m.height);
    let quality = |m: &Mode| (m.fps >= 15, !m.compressed, m.fps);
    let usable = || modes.iter().enumerate().filter(|(_, m)| side(m) > 0);
    let enough = usable()
        .filter(|(_, m)| side(m) >= max_side)
        .min_by(|(_, a), (_, b)| {
            (side(a), u64::from(a.width) * u64::from(a.height))
                .cmp(&(side(b), u64::from(b.width) * u64::from(b.height)))
                .then(quality(b).cmp(&quality(a)))
        });
    enough
        .or_else(|| {
            usable()
                .max_by(|(_, a), (_, b)| side(a).cmp(&side(b)).then(quality(a).cmp(&quality(b))))
        })
        .map(|(index, _)| index)
}

/// Kopiuje obraz 32-bitowy z bufora ciągłego do wierszy od góry bez dopełnień. `stride` < 0
/// oznacza wiersze od dołu (`MF_MT_DEFAULT_STRIDE`); `None`, gdy bufor jest za krótki.
pub(crate) fn rows_top_down(src: &[u8], stride: i32, width: u32, height: u32) -> Option<Vec<u8>> {
    let (row, rows) = (width as usize * 4, height as usize);
    let step = stride.unsigned_abs() as usize;
    if row == 0 || rows == 0 || step < row || src.len() < step * (rows - 1) + row {
        return None;
    }
    let mut out = Vec::with_capacity(row * rows);
    for y in 0..rows {
        let line = if stride < 0 { rows - 1 - y } else { y };
        out.extend_from_slice(&src[line * step..line * step + row]);
    }
    Some(out)
}

/// Ostatnia klatka kamery (BGRA, wiersze od góry) albo błąd kończący strumień.
#[derive(Debug, Default)]
struct Latest {
    seq: u64,
    frame: Option<(u32, u32, Vec<u8>)>,
    error: Option<PlatformError>,
}

/// Pamięć współdzielona między wątkiem kamery a strumieniem.
#[derive(Debug, Default)]
pub(crate) struct Shared {
    latest: Mutex<Latest>,
    fresh: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Latest> {
        self.latest.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Nowa klatka z wątku kamery (zastępuje poprzednią).
    pub(crate) fn publish(&self, width: u32, height: u32, bgra: Vec<u8>) {
        let mut latest = self.lock();
        latest.seq += 1;
        latest.frame = Some((width, height, bgra));
        self.fresh.notify_all();
    }

    /// Błąd kończący strumień (odłączenie, awaria sterownika).
    pub(crate) fn fail(&self, error: PlatformError) {
        self.lock().error = Some(error);
        self.fresh.notify_all();
    }
}

/// Strumień kamery po stronie wywołującego (wątek roboczy żyje do `Drop`).
pub(crate) struct WinStream {
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
    done: Receiver<()>,
    size: (u32, u32),
    max_side: u32,
    seen: u64,
}

impl WinStream {
    pub(crate) fn new(
        shared: Arc<Shared>,
        stop: Arc<AtomicBool>,
        done: Receiver<()>,
        native: (u32, u32),
        max_side: u32,
    ) -> Self {
        let max_side = max_side.max(1);
        Self {
            shared,
            stop,
            done,
            size: fitted(native, max_side),
            max_side,
            seen: 0,
        }
    }
}

/// Rozmiar po zmieszczeniu w `max_side` (ta sama arytmetyka co `RgbaImage::scale_to_fit`).
fn fitted(size: (u32, u32), max_side: u32) -> (u32, u32) {
    let (w, h, m) = (u64::from(size.0), u64::from(size.1), u64::from(max_side));
    if w <= m && h <= m {
        return size;
    }
    let (dw, dh) = if h <= w {
        (m, (h * m / w).max(1))
    } else {
        ((w * m / h).max(1), m)
    };
    (
        u32::try_from(dw).unwrap_or(1),
        u32::try_from(dh).unwrap_or(1),
    )
}

impl CameraStream for WinStream {
    fn read(&mut self, timeout: Duration) -> Result<RgbaImage, PlatformError> {
        let deadline = Instant::now() + timeout;
        let mut latest = self.shared.lock();
        loop {
            if let Some(error) = &latest.error {
                return Err(error.clone());
            }
            if latest.seq > self.seen {
                break;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(PlatformError::Io(
                    "kamera nie dostarczyła klatki w wyznaczonym czasie".into(),
                ));
            }
            latest = self
                .shared
                .fresh
                .wait_timeout(latest, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        self.seen = latest.seq;
        let frame = latest.frame.clone();
        drop(latest);
        let image = frame
            .and_then(|(w, h, bgra)| RgbaImage::from_bgra(w, h, bgra))
            .ok_or_else(|| PlatformError::Io("kamera podała klatkę o złym rozmiarze".into()))?;
        let image = image.scale_to_fit(self.max_side, self.max_side);
        self.size = (image.width, image.height);
        Ok(image)
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }
}

impl Drop for WinStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wątek kończy się po bieżącej klatce i zwalnia kamerę; zawieszonego sterownika nie
        // czekamy w nieskończoność (wątek zostaje porzucony, proces działa dalej).
        let _ = self.done.recv_timeout(CLOSE_WAIT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn mode(width: u32, height: u32, fps: u32, compressed: bool) -> Mode {
        Mode {
            width,
            height,
            fps,
            compressed,
        }
    }

    #[test]
    fn mode_closest_to_requested_side() {
        let modes = [
            mode(1920, 1080, 30, true),
            mode(1280, 720, 30, true),
            mode(1280, 720, 10, false),
            mode(640, 480, 30, false),
            mode(640, 480, 30, true),
            mode(320, 240, 30, false),
        ];
        assert_eq!(
            pick_mode(&modes, 640),
            Some(3),
            "nieskompresowany wygrywa remis"
        );
        assert_eq!(
            pick_mode(&modes, 1280),
            Some(1),
            "płynny przed nieskompresowanym"
        );
        assert_eq!(pick_mode(&modes, 200), Some(5));
        assert_eq!(
            pick_mode(&modes, 4000),
            Some(0),
            "brak większego → największy"
        );
        assert_eq!(pick_mode(&[], 640), None);
        assert_eq!(pick_mode(&[mode(0, 0, 30, false)], 640), None);
    }

    #[test]
    fn rows_are_flipped_for_negative_stride_and_padding_is_dropped() {
        // 1×2 px, krok 8 bajtów (4 bajty dopełnienia).
        let src = [1, 1, 1, 1, 9, 9, 9, 9, 2, 2, 2, 2, 9, 9, 9, 9];
        assert_eq!(
            rows_top_down(&src, 8, 1, 2).unwrap(),
            [1, 1, 1, 1, 2, 2, 2, 2]
        );
        assert_eq!(
            rows_top_down(&src, -8, 1, 2).unwrap(),
            [2, 2, 2, 2, 1, 1, 1, 1]
        );
        assert_eq!(rows_top_down(&src[..12], 8, 1, 2).unwrap().len(), 8);
        assert!(rows_top_down(&src[..11], 8, 1, 2).is_none());
        assert!(rows_top_down(&src, 2, 1, 2).is_none());
        assert!(rows_top_down(&src, 8, 0, 2).is_none());
    }

    #[test]
    fn stream_waits_for_a_newer_frame_and_reports_errors() {
        let shared = Arc::new(Shared::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (done_tx, done) = std::sync::mpsc::channel();
        let mut stream = WinStream::new(shared.clone(), stop.clone(), done, (4, 2), 2);
        assert_eq!(stream.size(), (2, 1));
        let short = Duration::from_millis(20);
        assert!(matches!(stream.read(short), Err(PlatformError::Io(_))));
        // BGRA → RGBA, pomniejszenie 4×2 → 2×1.
        shared.publish(4, 2, [10, 20, 30, 0].repeat(8));
        let image = stream.read(short).unwrap();
        assert_eq!((image.width, image.height), (2, 1));
        assert_eq!(image.pixel(0, 0), Some([30, 20, 10, 255]));
        assert!(
            stream.read(short).is_err(),
            "ta sama klatka nie wraca dwa razy"
        );
        let feeder = shared.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            feeder.publish(4, 2, vec![0; 32]);
        });
        assert!(stream.read(Duration::from_secs(5)).is_ok());
        thread.join().unwrap();
        shared.publish(4, 2, vec![0; 3]);
        assert!(stream.read(short).is_err(), "zły rozmiar bufora");
        shared.fail(PlatformError::UnknownResource("odłączona".into()));
        assert!(matches!(
            stream.read(short),
            Err(PlatformError::UnknownResource(_))
        ));
        done_tx.send(()).unwrap();
        drop(stream);
        assert!(stop.load(Ordering::SeqCst));
        assert_eq!(fitted((100, 400), 200), (50, 200));
        if !cfg!(windows) {
            assert!(WinCameras.devices().is_err() && WinCameras.open("x", 64).is_err());
        }
    }
}
