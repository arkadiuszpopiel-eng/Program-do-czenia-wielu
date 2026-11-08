//! Port kamer (docs/modules/camera-hub/SPEC.md): lista urządzeń wideo systemu i strumień klatek
//! RGBA. Implementacja Windows — Media Foundation w `platform-windows-impl`; atrapa — `platform-fake`.
//! Port niczego nie zapisuje na dysk; o tym, czy wolno otworzyć kamerę, decyduje `camera-hub`.

use std::time::Duration;

use crate::{PlatformError, RgbaImage};

/// Kamera widoczna w systemie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraDevice {
    /// Stabilny identyfikator systemowy (dowiązanie symboliczne urządzenia).
    pub id: String,
    /// Nazwa przyjazna z systemu.
    pub name: String,
}

/// Otwarty strumień kamery. Zwolnienie (`Drop`) zamyka kamerę.
pub trait CameraStream: Send {
    /// Jedna bieżąca klatka (wiersze od góry, RGBA). Kamera odłączona → `UnknownResource`.
    fn read(&mut self, timeout: Duration) -> Result<RgbaImage, PlatformError>;
    /// Rozmiar dostarczanych klatek (szerokość, wysokość).
    fn size(&self) -> (u32, u32);
}

/// Kamery systemu. Wywołania są blokujące — nie wołać na wątku środowiska asynchronicznego.
pub trait CameraPort: Send + Sync {
    /// Kamery podłączone w tej chwili (nie otwiera żadnej).
    fn devices(&self) -> Result<Vec<CameraDevice>, PlatformError>;
    /// Otwiera kamerę; dłuższy bok klatek nie przekracza `max_side` (tryb kamery albo pomniejszenie).
    fn open(&self, id: &str, max_side: u32) -> Result<Box<dyn CameraStream>, PlatformError>;
}
