//! Składanie tekstu stron: odstępy i nowe wiersze z położenia kolejnych napisów (w układzie
//! kierunku pisma — działa też dla obróconych stron), ligatury rozkładane, znaki sterujące
//! pomijane, nagłówki `--- Strona N ---`, limit znaków wyniku.

/// Koniec ostatniego napisu: punkt (układ urządzenia), kierunek pisma (wektor jednostkowy)
/// i rozmiar czcionki w układzie urządzenia.
#[derive(Debug, Clone, Copy)]
pub(super) struct Pen {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) ux: f64,
    pub(super) uy: f64,
    pub(super) size: f64,
}

/// Przesunięcie w poprzek wiersza (w rozmiarach czcionki), od którego zaczyna się nowy wiersz.
const NEW_LINE: f64 = 0.6;
/// Odstęp wzdłuż wiersza (w rozmiarach czcionki), od którego wstawiamy spację.
const WORD_GAP: f64 = 0.2;
/// Cofnięcie wzdłuż wiersza (w rozmiarach czcionki), które też rozdziela słowa (np. kolumny).
const BACK_GAP: f64 = 1.5;

/// Tekst dokumentu w budowie.
pub(super) struct Sink {
    out: String,
    page: String,
    chars: usize,
    limit: usize,
    pen: Option<Pen>,
    /// Osiągnięto limit znaków.
    pub(super) full: bool,
    /// Czy w dokumencie był jakikolwiek widoczny znak.
    pub(super) any_text: bool,
}

impl Sink {
    /// Pusty tekst z limitem `limit` znaków.
    pub(super) fn new(limit: usize) -> Self {
        Self {
            out: String::new(),
            page: String::new(),
            chars: 0,
            limit,
            pen: None,
            full: false,
            any_text: false,
        }
    }

    /// Początek napisu w punkcie `(x, y)`: spacja albo nowy wiersz względem końca poprzedniego.
    pub(super) fn start(&mut self, x: f64, y: f64) {
        let Some(p) = self.pen else { return };
        let (dx, dy) = (x - p.x, y - p.y);
        let along = dx * p.ux + dy * p.uy;
        let across = dy * p.ux - dx * p.uy;
        let size = if p.size > 1e-6 { p.size } else { 1.0 };
        if across.abs() > NEW_LINE * size {
            self.newline();
        } else if along > WORD_GAP * size || along < -BACK_GAP * size {
            self.put(' ');
        }
    }

    /// Koniec napisu.
    pub(super) fn end(&mut self, pen: Pen) {
        self.pen = Some(pen);
    }

    /// Tekst glifu.
    pub(super) fn push(&mut self, text: &str) {
        for c in text.chars() {
            match c {
                '\u{FB00}' => self.push_str("ff"),
                '\u{FB01}' => self.push_str("fi"),
                '\u{FB02}' => self.push_str("fl"),
                '\u{FB03}' => self.push_str("ffi"),
                '\u{FB04}' => self.push_str("ffl"),
                '\u{FB05}' | '\u{FB06}' => self.push_str("st"),
                '\t' | '\u{A0}' => self.put(' '),
                '\r' | '\n' => self.newline(),
                '\u{AD}' => {}
                c if c.is_control() => {}
                c => self.put(c),
            }
        }
    }

    fn push_str(&mut self, s: &str) {
        s.chars().for_each(|c| self.put(c));
    }

    fn put(&mut self, c: char) {
        if c == ' ' && (self.page.is_empty() || self.page.ends_with([' ', '\n'])) {
            return;
        }
        if self.chars >= self.limit {
            self.full = true;
            return;
        }
        self.page.push(c);
        self.chars += 1;
        if !c.is_whitespace() {
            self.any_text = true;
        }
    }

    fn newline(&mut self) {
        let kept = self.page.trim_end_matches(' ').len();
        self.page.truncate(kept);
        if !self.page.is_empty() && !self.page.ends_with('\n') {
            self.put('\n');
        }
    }

    /// Notka w tekście strony (np. pominięta treść) — poza limitem znaków.
    pub(super) fn note(&mut self, text: &str) {
        self.newline();
        self.page.push_str(text);
        self.page.push('\n');
    }

    /// Zamyka stronę `number`: nagłówek i tekst (albo notka o stronie bez tekstu).
    pub(super) fn page_done(&mut self, number: u32, had_image: bool) {
        let body = self.page.trim();
        let body = if !body.is_empty() {
            body
        } else if had_image {
            "[strona bez tekstu — obraz albo skan]"
        } else {
            "[pusta strona]"
        };
        let header = format!("--- Strona {number} ---\n");
        self.chars += header.chars().count() + 2;
        self.out.push_str(&header);
        self.out.push_str(body);
        self.out.push_str("\n\n");
        self.page.clear();
        self.pen = None;
    }

    /// Gotowy tekst.
    pub(super) fn finish(self) -> String {
        self.out.trim_end().to_owned()
    }
}
