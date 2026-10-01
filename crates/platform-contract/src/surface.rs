//! Natywna powierzchnia okna zatwierdzeń (Broker-UI, PLAN §8.2, ADR 3): czysty tekst, duże
//! przyciski, zawsze na wierzchu, bez WebView i bez renderowania HTML/markdown. Okno zgłasza
//! zdarzenia z próbką wejścia ([`InputSample`]: urządzenie, wstrzyknięcie, czas) i stanem
//! pierwszego planu — logika dowodu (500 ms na pierwszym planie, odrzucanie wstrzyknięć) jest
//! w `broker-ui`, nie tutaj.
//!
//! Klawiatura: `Tab` przenosi fokus, `Spacja` naciska przycisk z fokusem, `Esc` = [`SurfaceEvent::Cancel`];
//! `Enter` niczego nie zatwierdza (brak przycisku domyślnego).

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;
use crate::input::InputSample;

/// Maksymalna liczba przycisków.
pub const MAX_BUTTONS: usize = 6;
/// Najmniejszy dozwolony identyfikator przycisku (poniżej są `IDOK`/`IDCANCEL` itp.).
pub const MIN_BUTTON_ID: u16 = 100;
/// Limit długości pojedynczego tekstu (znaki).
pub const MAX_TEXT_CHARS: usize = 600;
/// Limit liczby wierszy szczegółów.
pub const MAX_DETAILS: usize = 16;

/// Ton (kolor) plakietki ryzyka — zawsze razem z tekstem i ikoną.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SurfaceTone {
    /// Neutralny.
    Neutral,
    /// Niskie ryzyko.
    Low,
    /// Średnie.
    Medium,
    /// Wysokie.
    High,
    /// Krytyczne.
    Critical,
}

/// Przycisk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceButton {
    /// Identyfikator (≥ [`MIN_BUTTON_ID`]).
    pub id: u16,
    /// Etykieta.
    pub label: String,
}

/// Zawartość okna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceView {
    /// Tytuł (pasek i nagłówek).
    pub title: String,
    /// Plakietka ryzyka (tekst z ikoną, np. „⚠ Ryzyko: wysokie”).
    pub badge: String,
    /// Kolor plakietki.
    pub tone: SurfaceTone,
    /// Wiersze „etykieta: wartość”.
    pub details: Vec<(String, String)>,
    /// Wiersz stanu (np. „Kliknij ponownie — okno dopiero stało się aktywne”).
    pub status: String,
    /// Przyciski (kolejność = kolejność `Tab`).
    pub buttons: Vec<SurfaceButton>,
    /// Przycisk z fokusem na starcie (bezpieczny: odmowa).
    pub initial_focus: u16,
    /// Czy okno może przejąć fokus (alertdialog przy wysokim ryzyku); inaczej pokazuje się bez
    /// aktywacji i sygnalizuje miganiem (nie kradnie fokusu w trakcie pisania).
    pub take_focus: bool,
}

impl SurfaceView {
    /// Walidacja: przyciski 1–6 o unikalnych id ≥ 100, fokus na istniejącym przycisku, limity
    /// tekstu, brak znaków sterujących (poza nową linią w szczegółach).
    pub fn validate(&self) -> Result<(), PlatformError> {
        let bad = |why: &str| Err(PlatformError::Unsupported(format!("widok okna: {why}")));
        if self.buttons.is_empty() || self.buttons.len() > MAX_BUTTONS {
            return bad("od 1 do 6 przycisków");
        }
        let mut ids: Vec<u16> = self.buttons.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != self.buttons.len() || ids.first().is_some_and(|&id| id < MIN_BUTTON_ID) {
            return bad("identyfikatory przycisków unikalne i ≥ 100");
        }
        if !ids.contains(&self.initial_focus) {
            return bad("fokus startowy poza przyciskami");
        }
        if self.details.len() > MAX_DETAILS {
            return bad("za dużo wierszy szczegółów");
        }
        let texts = [&self.title, &self.badge, &self.status]
            .into_iter()
            .chain(self.buttons.iter().map(|b| &b.label))
            .chain(self.details.iter().flat_map(|(k, v)| [k, v]));
        for t in texts {
            if t.chars().count() > MAX_TEXT_CHARS || t.chars().any(|c| c.is_control() && c != '\n')
            {
                return bad("tekst za długi albo ze znakami sterującymi");
            }
        }
        Ok(())
    }
}

/// Zdarzenie okna.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SurfaceEvent {
    /// Okno stało się oknem pierwszego planu.
    Activated {
        /// Chwila (ms).
        at_ms: u64,
    },
    /// Okno straciło pierwszy plan.
    Deactivated {
        /// Chwila (ms).
        at_ms: u64,
    },
    /// Naciśnięto przycisk.
    Button {
        /// Przycisk.
        id: u16,
        /// Próbka wejścia, które go nacisnęło.
        input: InputSample,
        /// Czy okno było zasłonięte innym oknem nad nim (ochrona przed nakładką).
        occluded: bool,
    },
    /// `Esc` albo zamknięcie okna przez użytkownika.
    Cancel {
        /// Próbka wejścia.
        input: InputSample,
    },
    /// Okno zniknęło bez udziału użytkownika (koniec sesji, błąd).
    Closed,
}

impl SurfaceTone {
    /// Kolor tekstu plakietki (RGB) — ciemne odcienie z kontrastem ≥ 4,5:1 na jasnym tle okna.
    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Neutral => (0x20, 0x20, 0x20),
            Self::Low => (0x1B, 0x5E, 0x20),
            Self::Medium => (0x8A, 0x4B, 0x00),
            Self::High => (0xB3, 0x26, 0x1E),
            Self::Critical => (0x7F, 0x00, 0x00),
        }
    }
}

/// Prostokąt w pikselach: (x, y, szerokość, wysokość).
pub type PixelRect = (i32, i32, i32, i32);

/// Układ okna w pikselach dla danego DPI (liczony przenośnie, rysowany przez Win32).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceLayout {
    /// Tytuł.
    pub title: PixelRect,
    /// Plakietka ryzyka.
    pub badge: PixelRect,
    /// Szczegóły (jeden blok tekstu `etykieta: wartość` w wierszach).
    pub details: PixelRect,
    /// Tekst szczegółów.
    pub details_text: String,
    /// Wiersz stanu.
    pub status: PixelRect,
    /// Przyciski (id, prostokąt) w kolejności `Tab`.
    pub buttons: Vec<(u16, PixelRect)>,
    /// Rozmiar okna (z ramką i paskiem tytułu).
    pub size: (i32, i32),
}

fn rows(text: &str) -> i32 {
    let lines = text.chars().count().div_ceil(64).max(1) + text.matches('\n').count();
    i32::try_from(lines).unwrap_or(1)
}

impl SurfaceView {
    /// Układ: szerokość 600 px przy 96 DPI, duże przyciski (48 px) w jednym rzędzie.
    pub fn layout(&self, dpi: u32) -> SurfaceLayout {
        let dpi = i32::try_from(dpi.clamp(96, 960)).unwrap_or(96);
        let s = |v: i32| v * dpi / 96;
        let (w, m, gap) = (s(600), s(16), s(6));
        let inner = w - 2 * m;
        let mut y = m;
        let mut next = |h: i32| {
            let r = (m, y, inner, h);
            y += h + gap;
            r
        };
        let title = next(s(30) * rows(&self.title));
        let badge = next(s(24));
        let details_text: String = self
            .details
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join("\n");
        let detail_rows: i32 = self
            .details
            .iter()
            .map(|(k, v)| rows(&format!("{k}: {v}")))
            .sum();
        let details = next(s(22) * detail_rows.max(1));
        let status = next(s(22) * rows(&self.status));
        let n = i32::try_from(self.buttons.len()).unwrap_or(1).max(1);
        let bw = (inner - (n - 1) * s(8)) / n;
        let buttons = self
            .buttons
            .iter()
            .zip(0..)
            .map(|(b, i)| (b.id, (m + i * (bw + s(8)), y, bw, s(48))))
            .collect();
        SurfaceLayout {
            title,
            badge,
            details,
            details_text,
            status,
            buttons,
            size: (w, y + s(48) + m + s(40)),
        }
    }
}

/// Port natywnego okna zatwierdzeń.
pub trait ApprovalSurfacePort: Send + Sync {
    /// Pokazuje (albo aktualizuje) okno z widokiem; widok jest walidowany.
    fn present(&self, view: &SurfaceView) -> Result<(), PlatformError>;

    /// Ukrywa okno.
    fn dismiss(&self) -> Result<(), PlatformError>;

    /// Następne zdarzenie (czeka do `timeout_ms`).
    fn next_event(&self, timeout_ms: u32) -> Option<SurfaceEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> SurfaceView {
        SurfaceView {
            title: "Delta prosi o zgodę".into(),
            badge: "⚠ Ryzyko: wysokie".into(),
            tone: SurfaceTone::High,
            details: vec![("Co".into(), "usuń 14 plików".into())],
            status: String::new(),
            buttons: vec![
                SurfaceButton {
                    id: 100,
                    label: "Odmów".into(),
                },
                SurfaceButton {
                    id: 101,
                    label: "Zezwól raz".into(),
                },
            ],
            initial_focus: 100,
            take_focus: false,
        }
    }

    #[test]
    fn view_validation() {
        assert!(view().validate().is_ok());
        let mut v = view();
        v.buttons[1].id = 100;
        assert!(v.validate().is_err());
        let mut v = view();
        v.buttons[0].id = 1;
        v.initial_focus = 1;
        assert!(v.validate().is_err(), "IDOK zarezerwowany");
        let mut v = view();
        v.initial_focus = 7;
        assert!(v.validate().is_err());
        let mut v = view();
        v.buttons.clear();
        assert!(v.validate().is_err());
        let mut v = view();
        v.title = "a\u{7}b".into();
        assert!(v.validate().is_err());
        let mut v = view();
        v.details[0].1 = "x".repeat(MAX_TEXT_CHARS + 1);
        assert!(v.validate().is_err());
        let mut v = view();
        v.details = vec![("a".into(), "b".into()); MAX_DETAILS + 1];
        assert!(v.validate().is_err());
        let mut v = view();
        v.details[0].1 = "wiele\nlinii".into();
        assert!(v.validate().is_ok());
    }

    #[test]
    fn layout_scales_with_dpi_and_keeps_tab_order() {
        let v = view();
        let l = v.layout(96);
        assert_eq!(l.size.0, 600);
        assert_eq!(
            l.buttons.iter().map(|b| b.0).collect::<Vec<_>>(),
            [100, 101]
        );
        assert!(l.buttons[0].1.0 < l.buttons[1].1.0, "przyciski w rzędzie");
        assert!(l.title.1 < l.badge.1 && l.badge.1 < l.details.1 && l.details.1 < l.status.1);
        assert_eq!(l.details_text, "Co: usuń 14 plików");
        let big = v.layout(192);
        assert_eq!(big.size.0, 1200);
        assert_eq!(big.buttons[0].1.3, 96);
        assert_eq!(v.layout(10).size.0, 600, "DPI poniżej 96 przycinane");
        assert_eq!(SurfaceTone::High.rgb(), (0xB3, 0x26, 0x1E));
    }
}
