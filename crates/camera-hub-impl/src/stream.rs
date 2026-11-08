//! Gniazdo strumienia jednej kamery i blokujące operacje na nim (wołane przez
//! `tokio::task::spawn_blocking`, nigdy na wątku środowiska asynchronicznego).
//!
//! Każde zatrzymanie (wyłączenie, `stop_all`, uśpienie) podnosi epokę gniazda. Żądanie klatki
//! pamięta epokę z chwili startu: gdy epoka się zmieni, żądanie kończy się `Stopped` i niczego
//! nie otwiera, a strumień otwarty przed zatrzymaniem zostaje zamknięty.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use camera_hub_contract::RgbaImage;
use platform_contract::{CameraPort, CameraStream, PlatformError};

/// Otwarty strumień z epoką, w której go otwarto.
type Open = (u64, Box<dyn CameraStream>);

/// Gniazdo strumienia kamery (jedno na kamerę; odczyty jednej kamery są szeregowane).
#[derive(Default)]
pub(crate) struct Slot {
    stream: Mutex<Option<Open>>,
    epoch: AtomicU64,
}

impl Slot {
    fn lock(&self) -> MutexGuard<'_, Option<Open>> {
        self.stream.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Bieżąca epoka.
    pub(crate) fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }

    /// Unieważnia żądania w toku; zwraca nową epokę (argument dla [`Slot::close_before`]).
    pub(crate) fn invalidate(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Zamyka strumień otwarty przed epoką `epoch` (blokuje do końca odczytu w toku).
    pub(crate) fn close_before(&self, epoch: u64) {
        let mut guard = self.lock();
        if guard.as_ref().is_some_and(|(opened, _)| *opened < epoch) {
            *guard = None;
        }
    }
}

/// Wynik nieudanego żądania klatki.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReadFailure {
    /// Zatrzymano w trakcie (epoka się zmieniła).
    Stopped,
    /// Kamera odłączona.
    Unplugged,
    /// Limit czasu odczytu.
    TimedOut(String),
    /// Inny błąd systemu.
    Failed(String),
}

/// Parametry odczytu.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReadPlan {
    /// Epoka z chwili przyjęcia żądania.
    pub epoch: u64,
    /// Dłuższy bok klatek przy otwieraniu kamery.
    pub capture_side: u32,
    /// Dłuższy bok klatki dla wywołującego.
    pub max_side: u32,
    /// Limit czekania na klatkę.
    pub timeout: Duration,
}

fn failure(error: PlatformError, waited: Duration, timeout: Duration) -> ReadFailure {
    match error {
        PlatformError::UnknownResource(_) => ReadFailure::Unplugged,
        PlatformError::Io(text) if waited >= timeout => ReadFailure::TimedOut(text),
        other => ReadFailure::Failed(other.to_string()),
    }
}

/// Otwiera kamerę, jeśli śpi, i czyta jedną klatkę. Zwraca klatkę pomniejszoną do `max_side`
/// oraz rozmiar klatek kamery. Błąd odczytu zamyka strumień (kolejne żądanie otworzy go od nowa).
pub(crate) fn read_frame(
    port: &dyn CameraPort,
    slot: &Slot,
    id: &str,
    plan: ReadPlan,
) -> Result<(RgbaImage, (u32, u32)), ReadFailure> {
    let mut guard = slot.lock();
    if slot.epoch() != plan.epoch {
        return Err(ReadFailure::Stopped);
    }
    // Strumień sprzed zatrzymania, którego zamykanie jeszcze nie dobiegło — zamykamy tutaj.
    if guard
        .as_ref()
        .is_some_and(|(opened, _)| *opened != plan.epoch)
    {
        *guard = None;
    }
    let started = Instant::now();
    if guard.is_none() {
        let stream = port
            .open(id, plan.capture_side)
            .map_err(|e| failure(e, Duration::ZERO, plan.timeout))?;
        if slot.epoch() != plan.epoch {
            return Err(ReadFailure::Stopped);
        }
        *guard = Some((plan.epoch, stream));
    }
    let Some((_, stream)) = guard.as_mut() else {
        return Err(ReadFailure::Failed("strumień kamery zniknął".into()));
    };
    let read = stream.read(plan.timeout);
    let size = stream.size();
    if slot.epoch() != plan.epoch {
        *guard = None;
        return Err(ReadFailure::Stopped);
    }
    match read {
        Ok(image) => Ok((image.scale_to_fit(plan.max_side, plan.max_side), size)),
        Err(error) => {
            *guard = None;
            Err(failure(error, started.elapsed(), plan.timeout))
        }
    }
}

#[cfg(test)]
mod tests {
    use platform_fake::FakeCameras;

    use super::*;

    fn plan(epoch: u64) -> ReadPlan {
        ReadPlan {
            epoch,
            capture_side: 320,
            max_side: 32,
            timeout: Duration::from_millis(50),
        }
    }

    #[test]
    fn stale_requests_never_open_and_old_streams_are_closed() {
        let cams = FakeCameras::new(1);
        let slot = Slot::default();
        let id = "fake-cam-0";
        let (image, size) = read_frame(&cams, &slot, id, plan(0)).unwrap();
        assert_eq!((image.width, image.height, size), (32, 24, (320, 240)));
        assert_eq!(cams.open_streams(id), 1);
        // Zatrzymanie: żądanie ze starą epoką nie czyta, a strumień sprzed epoki znika.
        let epoch = slot.invalidate();
        assert_eq!(
            read_frame(&cams, &slot, id, plan(0)),
            Err(ReadFailure::Stopped)
        );
        assert_eq!(cams.frames_read(id), 1);
        // Nowe żądanie przed zamknięciem: stary strumień zamknięty, nowy otwarty.
        assert!(read_frame(&cams, &slot, id, plan(epoch)).is_ok());
        assert_eq!((cams.open_streams(id), cams.opened_count(id)), (1, 2));
        slot.close_before(epoch);
        assert_eq!(cams.open_streams(id), 1, "strumień nowej epoki zostaje");
        slot.close_before(slot.invalidate());
        assert_eq!(cams.open_streams(id), 0);
        assert_eq!(
            read_frame(&cams, &slot, id, plan(epoch)),
            Err(ReadFailure::Stopped)
        );
        assert_eq!(
            cams.opened_count(id),
            2,
            "stare żądanie niczego nie otwiera"
        );
    }

    #[test]
    fn failures_close_the_stream_and_are_classified() {
        let cams = FakeCameras::new(1);
        let slot = Slot::default();
        let id = "fake-cam-0";
        cams.fail_open(id, Some("zajęta"));
        let failed = read_frame(&cams, &slot, id, plan(0));
        assert!(matches!(failed, Err(ReadFailure::Failed(m)) if m.contains("zajęta")));
        cams.fail_open(id, None);
        assert!(read_frame(&cams, &slot, id, plan(0)).is_ok());
        cams.fail_read(id, Some("usterka"));
        assert!(matches!(
            read_frame(&cams, &slot, id, plan(0)),
            Err(ReadFailure::Failed(_))
        ));
        assert_eq!(cams.open_streams(id), 0, "błąd odczytu zamyka strumień");
        cams.set_delay(Duration::from_millis(60));
        assert!(matches!(
            read_frame(&cams, &slot, id, plan(0)),
            Err(ReadFailure::TimedOut(_))
        ));
        cams.set_delay(Duration::ZERO);
        cams.fail_read(id, None);
        cams.unplug(id);
        assert_eq!(
            read_frame(&cams, &slot, id, plan(0)),
            Err(ReadFailure::Unplugged)
        );
        let io = PlatformError::PermissionDenied("odmowa".into());
        assert!(matches!(
            failure(io, Duration::from_secs(9), Duration::from_secs(1)),
            ReadFailure::Failed(_)
        ));
    }
}
