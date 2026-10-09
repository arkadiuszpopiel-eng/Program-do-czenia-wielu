//! Kontrakt modułu `camera-hub` (docs/modules/camera-hub/SPEC.md, wersja v0: kamery USB).
//!
//! Hub jest jedynym miejscem, które otwiera kamery. Reguły: przełącznik główny wyłączony
//! (domyślnie) albo kamera wyłączona ⇒ strumień nie otwiera się nigdy; strumień śpi i otwiera
//! się dopiero na żądanie klatki, a po okresie bezczynności zamyka się sam; `stop_all` i
//! wyłączenie przełącznika zamykają wszystko od razu i wygrywają z żądaniami w toku. Żadna
//! klatka nie trafia na dysk.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(feature = "contract-tests")]
pub mod contract_tests;
mod store;

use async_trait::async_trait;
pub use platform_contract::RgbaImage;
use tokio::sync::watch;

pub use store::{CameraStore, MemoryCameraStore, StoredCamera, StoredCameras};

/// Najmniejszy dopuszczalny `max_side` żądania klatki.
pub const MIN_FRAME_SIDE: u32 = 16;
/// Największa długość nazwy kamery nadanej przez użytkownika (znaki).
pub const MAX_NAME_CHARS: usize = 64;

/// Identyfikator kamery (dla USB: stabilny identyfikator systemowy urządzenia).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CameraId(pub String);

impl CameraId {
    /// Widok tekstowy.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CameraId {
    fn from(id: &str) -> Self {
        Self(id.to_owned())
    }
}

/// Rodzaj kamery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraKind {
    /// Kamera systemu (USB, wbudowana).
    Usb,
    /// Kamera sieciowa (v1 — w v0 nie występuje).
    Network,
}

/// Stan strumienia kamery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CameraState {
    /// Zamknięta; otworzy się na żądanie klatki.
    Asleep,
    /// W trakcie otwierania.
    Opening,
    /// Otwarta — dostarcza klatki o tym rozmiarze (przed pomniejszeniem na żądanie).
    Live {
        /// Szerokość klatki kamery.
        width: u32,
        /// Wysokość klatki kamery.
        height: u32,
    },
    /// Ostatnie otwarcie albo odczyt nie powiodły się (opis po polsku); kolejne żądanie ponawia.
    Failed(String),
    /// Kamera odłączona od systemu.
    Unplugged,
}

/// Kamera w rejestrze huba.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraInfo {
    /// Identyfikator.
    pub id: CameraId,
    /// Nazwa nadana przez użytkownika albo nazwa systemowa.
    pub name: String,
    /// Rodzaj.
    pub kind: CameraKind,
    /// Przełącznik tej kamery (wyłączona nie otwiera się nigdy).
    pub enabled: bool,
    /// Stan strumienia.
    pub state: CameraState,
}

/// Stan huba: przełącznik główny i kamery.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HubState {
    /// Przełącznik główny (domyślnie wyłączony).
    pub enabled: bool,
    /// Kamery widziane w tej sesji programu (w kolejności wykrycia).
    pub cameras: Vec<CameraInfo>,
}

impl HubState {
    /// Kamera o podanym identyfikatorze.
    pub fn camera(&self, id: &CameraId) -> Option<&CameraInfo> {
        self.cameras.iter().find(|c| &c.id == id)
    }

    /// Czy którakolwiek kamera jest otwarta albo właśnie się otwiera (wskaźnik w interfejsie).
    pub fn any_open(&self) -> bool {
        self.cameras
            .iter()
            .any(|c| matches!(c.state, CameraState::Opening | CameraState::Live { .. }))
    }
}

/// Klatka z kamery (żyje tylko w pamięci).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Kamera, z której pochodzi.
    pub camera: CameraId,
    /// Chwila pobrania (milisekundy od 1970-01-01 UTC).
    pub taken_ms: u64,
    /// Obraz RGBA, wiersze od góry, dłuższy bok ≤ żądany `max_side`.
    pub image: RgbaImage,
}

/// Błąd huba kamer.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum CameraError {
    /// Przełącznik główny albo przełącznik tej kamery jest wyłączony.
    #[error("kamery są wyłączone albo ta kamera jest wyłączona")]
    Disabled,
    /// Nie ma takiej kamery w rejestrze.
    #[error("nieznana kamera: {0}")]
    UnknownCamera(String),
    /// Kamera jest odłączona.
    #[error("kamera jest odłączona")]
    Unplugged,
    /// Żądanie przerwane przez `stop_all`, wyłączenie kamery albo przełącznika głównego.
    #[error("kamera została zatrzymana w trakcie żądania")]
    Stopped,
    /// Kamera nie dostarczyła klatki w wyznaczonym czasie.
    #[error("kamera nie odpowiedziała w wyznaczonym czasie")]
    Timeout,
    /// Błąd systemu albo sterownika (opis po polsku).
    #[error("błąd kamery: {0}")]
    Platform(String),
    /// Nie udało się zapisać ustawień kamer.
    #[error("nie można zapisać ustawień kamer: {0}")]
    Store(String),
}

/// Hub kamer.
#[async_trait]
pub trait CameraHub: Send + Sync {
    /// Bieżący stan (bez wyliczania urządzeń).
    async fn state(&self) -> HubState;
    /// Przełącznik główny; wyłączenie zamyka wszystkie strumienie od razu.
    async fn set_master(&self, enabled: bool) -> Result<HubState, CameraError>;
    /// Przełącznik jednej kamery; wyłączenie zamyka jej strumień od razu.
    async fn set_enabled(&self, id: &CameraId, enabled: bool) -> Result<HubState, CameraError>;
    /// Nazwa nadana przez użytkownika; pusta przywraca nazwę systemową.
    async fn rename(&self, id: &CameraId, name: &str) -> Result<HubState, CameraError>;
    /// Jedna bieżąca klatka (budzi kamerę); dłuższy bok ≤ `max_side`.
    async fn frame(&self, id: &CameraId, max_side: u32) -> Result<Frame, CameraError>;
    /// Zamyka wszystkie strumienie od razu (kill-switch); przełączników nie zmienia.
    async fn stop_all(&self);
    /// Ponowne wyliczenie urządzeń systemu (podłączenie i odłączenie kamer).
    async fn refresh(&self) -> Result<HubState, CameraError>;
    /// Obserwacja stanu: nowa wartość po każdej zmianie listy, stanu, przełącznika albo nazwy.
    fn changes(&self) -> watch::Receiver<HubState>;
}

/// Nazwa kamery po oczyszczeniu: bez znaków sterujących i skrajnych spacji, najwyżej
/// [`MAX_NAME_CHARS`] znaków; `None` = pusta (przywróć nazwę systemową).
pub fn clean_name(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let cleaned = cleaned.trim_end().to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_cleaned_and_state_helpers_work() {
        assert_eq!(clean_name("  kuchnia\n "), Some("kuchnia".to_owned()));
        assert_eq!(clean_name(" \t "), None);
        assert_eq!(
            clean_name(&"ż".repeat(200)).map(|n| n.chars().count()),
            Some(MAX_NAME_CHARS)
        );
        let camera = |id: &str, state| CameraInfo {
            id: id.into(),
            name: id.to_owned(),
            kind: CameraKind::Usb,
            enabled: true,
            state,
        };
        let mut hub = HubState {
            enabled: true,
            cameras: vec![camera("a", CameraState::Asleep)],
        };
        assert!(!hub.any_open());
        assert!(hub.camera(&"a".into()).is_some() && hub.camera(&"b".into()).is_none());
        hub.cameras.push(camera(
            "b",
            CameraState::Live {
                width: 4,
                height: 2,
            },
        ));
        assert!(hub.any_open());
        assert_eq!(CameraId::from("x").as_str(), "x");
        assert!(
            CameraError::UnknownCamera("x".into())
                .to_string()
                .contains('x')
        );
    }
}
