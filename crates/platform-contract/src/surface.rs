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

/// Układ okna w pikselach dla danego DPI (liczony przenośnie, rysowany przez Win32). Wzór: karta
/// zgody z `ui-kit` (`ApprovalCard`): karta z obramowaniem i paskiem ryzyka po lewej na tle okna,
/// nagłówek, siatka „etykieta / wartość”, podpowiedź i przyciski w stopce karty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceLayout {
    /// Karta (tło `surface`, obramowanie, pasek ryzyka po lewej).
    pub card: PixelRect,
    /// Tytuł.
    pub title: PixelRect,
    /// Plakietka ryzyka.
    pub badge: PixelRect,
    /// Wiersze szczegółów: (etykieta, wartość) — w kolejności `details`.
    pub rows: Vec<(PixelRect, PixelRect)>,
    /// Wiersz stanu (podpowiedź klawiatury albo powód odrzucenia).
    pub status: PixelRect,
    /// Przyciski (id, prostokąt) w kolejności `Tab`.
    pub buttons: Vec<(u16, PixelRect)>,
    /// Rozmiar okna (z ramką i paskiem tytułu).
    pub size: (i32, i32),
}

/// Liczba linii tekstu przy `per_line` znakach w linii (z jawnymi znakami nowej linii).
fn lines(text: &str, per_line: usize) -> i32 {
    let count: usize = text
        .split('\n')
        .map(|l| l.chars().count().div_ceil(per_line).max(1))
        .sum();
    i32::try_from(count.max(1)).unwrap_or(1)
}

/// Szerokość kolumny etykiet (px przy 96 DPI) — jak siatka `ApprovalCard` (`max-content`).
const LABEL_WIDTH: i32 = 150;

impl SurfaceView {
    /// Układ: okno 600 px szerokości przy 96 DPI; odstępy z tokenów `ui-kit` (4/8/12/16/24 px),
    /// przyciski 36 px w jednym rzędzie w stopce karty.
    pub fn layout(&self, dpi: u32) -> SurfaceLayout {
        let dpi = i32::try_from(dpi.clamp(96, 960)).unwrap_or(96);
        let s = |v: i32| v * dpi / 96;
        // Okno: ramka systemu ok. 16 px; margines tła 16; karta: pasek 3 + wcięcie 16.
        let w = s(600);
        let (margin, pad, stripe) = (s(16), s(16), s(3));
        let card_w = w - s(16) - 2 * margin;
        let x = margin + stripe + pad;
        let inner = card_w - stripe - 2 * pad;
        let mut y = margin + pad;
        let title = (x, y, inner, s(28) * lines(&self.title, 40));
        y += title.3 + s(4);
        let badge = (x, y, inner, s(22));
        y += badge.3 + s(12);
        let value_x = x + s(LABEL_WIDTH) + s(12);
        let value_w = inner - s(LABEL_WIDTH) - s(12);
        let rows = self
            .details
            .iter()
            .map(|(k, v)| {
                let h = s(20) * lines(v, 46).max(lines(k, 18));
                let row = ((x, y, s(LABEL_WIDTH), h), (value_x, y, value_w, h));
                y += h + s(4);
                row
            })
            .collect();
        y += s(8);
        let status = (x, y, inner, s(18) * lines(&self.status, 80));
        y += status.3 + s(16);
        let n = i32::try_from(self.buttons.len()).unwrap_or(1).max(1);
        let bw = (inner - (n - 1) * s(8)) / n;
        let buttons = self
            .buttons
            .iter()
            .zip(0..)
            .map(|(b, i)| (b.id, (x + i * (bw + s(8)), y, bw, s(36))))
            .collect();
        y += s(36) + pad;
        let card = (margin, margin, card_w, y - margin);
        SurfaceLayout {
            card,
            title,
            badge,
            rows,
            status,
            buttons,
            size: (w, y + margin + s(40)),
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
        let row = l.rows[0];
        assert!(l.title.1 < l.badge.1 && l.badge.1 < row.0.1 && row.0.1 < l.status.1);
        assert_eq!(row.0.1, row.1.1, "etykieta i wartość w jednym wierszu");
        assert!(row.0.0 + row.0.2 < row.1.0, "wartość na prawo od etykiety");
        let (cx, cy, cw, ch) = l.card;
        for (_, (bx, by, bw, bh)) in &l.buttons {
            assert!(
                *bx >= cx && bx + bw <= cx + cw && *by >= cy && by + bh <= cy + ch,
                "przyciski w karcie"
            );
        }
        let big = v.layout(192);
        assert_eq!(big.size.0, 1200);
        assert_eq!(big.buttons[0].1.3, 72);
        assert_eq!(v.layout(10).size.0, 600, "DPI poniżej 96 przycinane");
        assert_eq!(SurfaceTone::High.rgb(), (0xB3, 0x26, 0x1E));
    }

    #[test]
    fn long_values_get_more_lines() {
        let mut v = view();
        let short = v.layout(96).rows[0].1.3;
        v.details[0].1 = "x".repeat(200);
        assert!(v.layout(96).rows[0].1.3 > short);
        v.details[0].1 = "a\nb\nc".into();
        assert_eq!(v.layout(96).rows[0].1.3, 3 * short);
    }
}
