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
/// Limit tekstu pomocy pod „?” (znaki).
pub const MAX_HELP_CHARS: usize = 1_200;

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

/// Rząd wyboru pod przyciskami głównymi (np. „Zawsze w tym zakresie: 1 h · 3 h · 24 h · Do
/// odwołania”) — każdy przycisk rzędu to osobna decyzja; okno rysuje je mniejsze, z etykietą.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceChoice {
    /// Etykieta rzędu (też początek nazwy dostępnej każdego przycisku).
    pub label: String,
    /// Przyciski rzędu (identyfikatory z `SurfaceView::buttons`).
    pub buttons: Vec<u16>,
}

/// Zawartość okna.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceView {
    /// Tytuł (pasek i nagłówek).
    pub title: String,
    /// Plakietki w nagłówku oddzielone „ · ” (np. „Ryzyko wysokie · zlecone głosem”); pierwsza
    /// w kolorze ryzyka, ikonę ryzyka rysuje okno według `tone`.
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
    /// Pomoc pod przyciskiem „?” (co to za okno i do czego się odnosi); pusta — bez „?”.
    #[serde(default)]
    pub help: String,
    /// Rząd wyboru (np. czas zgody „zawsze w tym zakresie”); brak — same przyciski główne.
    #[serde(default)]
    pub choice: Option<SurfaceChoice>,
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
        if self.help.chars().count() > MAX_HELP_CHARS || self.help.chars().any(char::is_control) {
            return bad("pomoc za długa albo ze znakami sterującymi");
        }
        if let Some(c) = &self.choice {
            let known = c.buttons.iter().all(|id| ids.contains(id));
            if c.buttons.is_empty() || !known || c.buttons.contains(&self.initial_focus) {
                return bad("rząd wyboru: istniejące przyciski, bez przycisku startowego");
            }
        }
        let choice = self.choice.as_ref().map(|c| &c.label);
        let texts = [&self.title, &self.badge, &self.status]
            .into_iter()
            .chain(choice)
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
