//! Atrapa `CameraPort`: wirtualne kamery z deterministycznym, ruchomym wzorem testowym (numer
//! klatki zmienia piksele). Test steruje odłączeniem, błędami otwarcia i odczytu oraz opóźnieniem,
//! a liczniki otwartych strumieni pozwalają sprawdzić, że kamera została naprawdę zamknięta.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use platform_contract::{CameraDevice, CameraPort, CameraStream, PlatformError, RgbaImage};

/// Domyślny rozmiar klatki wirtualnej kamery.
const NATIVE: (u32, u32) = (640, 480);

#[derive(Debug, Clone)]
struct Cam {
    name: String,
    size: (u32, u32),
    plugged: bool,
    fail_open: Option<String>,
    fail_read: Option<String>,
    open_now: usize,
    opened_total: usize,
    frames: u64,
}

#[derive(Debug, Default)]
struct State {
    cams: BTreeMap<String, Cam>,
    delay: Duration,
    fail_devices: Option<String>,
}

/// Wirtualne kamery (tanie do klonowania; klony dzielą stan).
#[derive(Debug, Clone, Default)]
pub struct FakeCameras {
    state: Arc<Mutex<State>>,
}

impl FakeCameras {
    /// `count` kamer `fake-cam-0…` o nazwach „Kamera testowa 1…” i klatkach 640×480.
    pub fn new(count: usize) -> Self {
        let fake = Self::default();
        for i in 0..count {
            fake.add(
                &format!("fake-cam-{i}"),
                &format!("Kamera testowa {}", i + 1),
                NATIVE,
            );
        }
        fake
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn with_cam(&self, id: &str, change: impl FnOnce(&mut Cam)) {
        if let Some(cam) = self.lock().cams.get_mut(id) {
            change(cam);
        }
    }

    /// Dodaje (podłącza) kamerę o podanym rozmiarze klatki.
    pub fn add(&self, id: &str, name: &str, size: (u32, u32)) {
        let cam = Cam {
            name: name.to_owned(),
            size: (size.0.max(1), size.1.max(1)),
            plugged: true,
            fail_open: None,
            fail_read: None,
            open_now: 0,
            opened_total: 0,
            frames: 0,
        };
        self.lock().cams.insert(id.to_owned(), cam);
    }

    /// Odłącza kamerę: znika z listy, otwarte strumienie zwracają `UnknownResource`.
    pub fn unplug(&self, id: &str) {
        self.with_cam(id, |c| c.plugged = false);
    }

    /// Podłącza kamerę z powrotem.
    pub fn plug(&self, id: &str) {
        self.with_cam(id, |c| c.plugged = true);
    }

    /// Błąd przy otwieraniu (`None` — otwieranie działa).
    pub fn fail_open(&self, id: &str, error: Option<&str>) {
        self.with_cam(id, |c| c.fail_open = error.map(str::to_owned));
    }

    /// Błąd przy odczycie klatki (`None` — odczyt działa).
    pub fn fail_read(&self, id: &str, error: Option<&str>) {
        self.with_cam(id, |c| c.fail_read = error.map(str::to_owned));
    }

    /// Błąd wyliczania urządzeń (`None` — lista działa).
    pub fn fail_devices(&self, error: Option<&str>) {
        self.lock().fail_devices = error.map(str::to_owned);
    }

    /// Opóźnienie każdego otwarcia i odczytu (czas rzeczywisty; domyślnie brak).
    pub fn set_delay(&self, delay: Duration) {
        self.lock().delay = delay;
    }

    /// Liczba otwartych teraz strumieni kamery.
    pub fn open_streams(&self, id: &str) -> usize {
        self.lock().cams.get(id).map_or(0, |c| c.open_now)
    }

    /// Liczba otwartych teraz strumieni wszystkich kamer.
    pub fn open_total(&self) -> usize {
        self.lock().cams.values().map(|c| c.open_now).sum()
    }

    /// Ile razy kamera była otwierana od początku.
    pub fn opened_count(&self, id: &str) -> usize {
        self.lock().cams.get(id).map_or(0, |c| c.opened_total)
    }

    /// Ile klatek kamera wydała od początku.
    pub fn frames_read(&self, id: &str) -> u64 {
        self.lock().cams.get(id).map_or(0, |c| c.frames)
    }

    fn pause(&self) {
        let delay = self.lock().delay;
        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
    }
}

fn missing(id: &str) -> PlatformError {
    PlatformError::UnknownResource(format!("kamera „{id}” nie jest podłączona"))
}

/// Rozmiar po zmieszczeniu w `max_side` (jak `RgbaImage::scale_to_fit`; bez powiększania).
fn fitted(size: (u32, u32), max_side: u32) -> (u32, u32) {
    let (w, h) = (u64::from(size.0), u64::from(size.1));
    let m = u64::from(max_side.max(1));
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

/// Wzór testowy: gradient zależny od kamery i przesuwający się z numerem klatki.
fn pattern(size: (u32, u32), seed: u8, frame: u64) -> RgbaImage {
    let (w, h) = size;
    let shift = u32::try_from(frame % 256).unwrap_or(0);
    let mut pixels = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let r = u8::try_from((x + shift) % 256).unwrap_or(0);
            let g = u8::try_from((y + shift * 2) % 256).unwrap_or(0);
            pixels.extend_from_slice(&[r, g, seed, 255]);
        }
    }
    RgbaImage {
        width: w,
        height: h,
        pixels,
    }
}

impl CameraPort for FakeCameras {
    fn devices(&self) -> Result<Vec<CameraDevice>, PlatformError> {
        let state = self.lock();
        if let Some(error) = &state.fail_devices {
            return Err(PlatformError::Io(error.clone()));
        }
        Ok(state
            .cams
            .iter()
            .filter(|(_, c)| c.plugged)
            .map(|(id, c)| CameraDevice {
                id: id.clone(),
                name: c.name.clone(),
            })
            .collect())
    }

    fn open(&self, id: &str, max_side: u32) -> Result<Box<dyn CameraStream>, PlatformError> {
        self.pause();
        let mut state = self.lock();
        let index = state.cams.keys().position(|k| k == id).unwrap_or(0);
        let cam = state.cams.get_mut(id).ok_or_else(|| missing(id))?;
        if !cam.plugged {
            return Err(missing(id));
        }
        if let Some(error) = &cam.fail_open {
            return Err(PlatformError::Io(error.clone()));
        }
        cam.open_now += 1;
        cam.opened_total += 1;
        Ok(Box::new(FakeStream {
            owner: self.clone(),
            id: id.to_owned(),
            size: fitted(cam.size, max_side),
            seed: u8::try_from(index * 40 % 256).unwrap_or(0),
        }))
    }
}

/// Strumień wirtualnej kamery.
#[derive(Debug)]
struct FakeStream {
    owner: FakeCameras,
    id: String,
    size: (u32, u32),
    seed: u8,
}

impl CameraStream for FakeStream {
    fn read(&mut self, _timeout: Duration) -> Result<RgbaImage, PlatformError> {
        self.owner.pause();
        let mut state = self.owner.lock();
        let cam = state
            .cams
            .get_mut(&self.id)
            .ok_or_else(|| missing(&self.id))?;
        if !cam.plugged {
            return Err(missing(&self.id));
        }
        if let Some(error) = &cam.fail_read {
            return Err(PlatformError::Io(error.clone()));
        }
        cam.frames += 1;
        Ok(pattern(self.size, self.seed, cam.frames))
    }

    fn size(&self) -> (u32, u32) {
        self.size
    }
}

impl Drop for FakeStream {
    fn drop(&mut self) {
        self.owner
            .with_cam(&self.id, |c| c.open_now = c.open_now.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Duration = Duration::from_millis(100);

    #[test]
    fn frames_move_and_streams_are_counted() {
        let cams = FakeCameras::new(2);
        assert_eq!(cams.devices().unwrap().len(), 2);
        let mut a = cams.open("fake-cam-0", 320).unwrap();
        let mut b = cams.open("fake-cam-1", 4000).unwrap();
        assert_eq!((a.size(), b.size()), ((320, 240), (640, 480)));
        let (first, second) = (a.read(T).unwrap(), a.read(T).unwrap());
        assert_eq!((first.width, first.height), (320, 240));
        assert_ne!(first.pixels, second.pixels, "numer klatki zmienia piksele");
        assert_ne!(b.read(T).unwrap().pixel(0, 0), first.pixel(0, 0));
        assert_eq!((cams.open_total(), cams.open_streams("fake-cam-0")), (2, 1));
        drop(a);
        assert_eq!((cams.open_total(), cams.opened_count("fake-cam-0")), (1, 1));
        assert_eq!(cams.frames_read("fake-cam-0"), 2);
        drop(b);
        assert_eq!(cams.open_total(), 0);
    }

    #[test]
    fn unplug_and_failures_are_scripted() {
        let cams = FakeCameras::new(1);
        let mut s = cams.open("fake-cam-0", 64).unwrap();
        assert_eq!(s.size(), (64, 48));
        cams.fail_read("fake-cam-0", Some("usterka"));
        assert_eq!(s.read(T), Err(PlatformError::Io("usterka".into())));
        cams.fail_read("fake-cam-0", None);
        cams.unplug("fake-cam-0");
        assert!(matches!(s.read(T), Err(PlatformError::UnknownResource(_))));
        assert!(cams.devices().unwrap().is_empty());
        assert!(cams.open("fake-cam-0", 64).is_err());
        cams.plug("fake-cam-0");
        cams.fail_open("fake-cam-0", Some("zajęta"));
        assert!(matches!(cams.open("fake-cam-0", 64), Err(PlatformError::Io(m)) if m == "zajęta"));
        assert!(cams.open("nie-ma", 64).is_err());
        cams.fail_devices(Some("brak usługi"));
        assert!(cams.devices().is_err());
        cams.add("pion", "Pionowa", (100, 400));
        assert_eq!(cams.open("pion", 200).unwrap().size(), (50, 200));
    }
}
