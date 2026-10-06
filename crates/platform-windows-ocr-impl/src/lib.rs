//! `OcrPort` przez `Windows.Media.Ocr` (docs/modules/tools-vision/SPEC.md, PLAN §7.2 „GUI: OCR”).
//!
//! Obraz (PNG z zamaskowanego zrzutu albo plik po kontroli nagłówka) → `InMemoryRandomAccessStream`
//! → `BitmapDecoder` (wymiary sprawdzane **przed** dekodowaniem pikseli — limit liczby pikseli)
//! → `SoftwareBitmap` BGRA8 przeskalowany do `OcrEngine::MaxImageDimension` → `OcrEngine`
//! (język z żądania albo z profilu użytkownika) → linie i słowa ze współrzędnymi przeliczonymi
//! na piksele obrazu wejściowego. Wywołania blokujące (`join`) — z kodu async przez `spawn_blocking`.
//! Poza Windows port zwraca [`OcrError::Unsupported`].

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

#[cfg(windows)]
mod win;

use tools_vision_contract::{OcrError, OcrPort, OcrRequest, OcrText};

/// Domyślny limit liczby pikseli obrazu przed dekodowaniem (ok. 40 Mpx).
pub const DEFAULT_MAX_PIXELS: u64 = 40_000_000;

/// OCR systemu Windows.
#[derive(Debug, Clone, Copy)]
pub struct WinOcr {
    max_pixels: u64,
}

impl Default for WinOcr {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_PIXELS)
    }
}

impl WinOcr {
    /// Port z limitem liczby pikseli obrazu.
    pub fn new(max_pixels: u64) -> Self {
        Self { max_pixels }
    }

    /// Limit liczby pikseli.
    pub fn max_pixels(&self) -> u64 {
        self.max_pixels
    }
}

/// Komunikat poza Windows.
pub const UNSUPPORTED: &str = "Windows.Media.Ocr wymaga Windows 10/11";

impl OcrPort for WinOcr {
    fn recognize(&self, request: &OcrRequest) -> Result<OcrText, OcrError> {
        if request.image.is_empty() {
            return Err(OcrError::Image("pusty obraz".into()));
        }
        #[cfg(windows)]
        {
            win::recognize(request, self.max_pixels)
        }
        #[cfg(not(windows))]
        {
            Err(OcrError::Unsupported(UNSUPPORTED.into()))
        }
    }

    fn languages(&self) -> Result<Vec<String>, OcrError> {
        #[cfg(windows)]
        {
            win::languages()
        }
        #[cfg(not(windows))]
        {
            Err(OcrError::Unsupported(UNSUPPORTED.into()))
        }
    }
}

/// Współczynnik skalowania obrazu `w`×`h` do dłuższego boku `max_side` (≤ 1 — tylko pomniejszanie).
pub fn fit_factor(w: u32, h: u32, max_side: u32) -> f64 {
    let longest = w.max(h);
    if longest == 0 || max_side == 0 || longest <= max_side {
        1.0
    } else {
        f64::from(max_side) / f64::from(longest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_factor_only_shrinks() {
        assert_eq!(fit_factor(800, 600, 2600), 1.0);
        assert!((fit_factor(5200, 1000, 2600) - 0.5).abs() < 1e-9);
        assert_eq!(fit_factor(0, 0, 2600), 1.0);
        assert_eq!(fit_factor(10, 10, 0), 1.0);
    }

    #[test]
    fn port_rejects_empty_and_reports_platform() {
        let ocr = WinOcr::default();
        assert_eq!(ocr.max_pixels(), DEFAULT_MAX_PIXELS);
        let empty = OcrRequest {
            image: Vec::new(),
            language: None,
        };
        assert!(matches!(ocr.recognize(&empty), Err(OcrError::Image(_))));
        #[cfg(not(windows))]
        {
            let png = OcrRequest {
                image: vec![1, 2, 3],
                language: Some("pl".into()),
            };
            assert_eq!(
                ocr.recognize(&png),
                Err(OcrError::Unsupported(UNSUPPORTED.into()))
            );
            assert!(ocr.languages().is_err());
        }
    }
}
