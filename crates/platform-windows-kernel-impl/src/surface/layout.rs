//! Układ okna Sędziego Cieni mierzony prawdziwą czcionką (`DrawTextW` + `DT_CALCRECT`): wysokość
//! każdego tekstu wynika z łamania wierszy w danej szerokości, a długie słowa bez spacji (ścieżki,
//! adresy) łamią się w środku (`DT_EDITCONTROL`) — nic nie wychodzi poza okno i nic się nie
//! nakłada. Okno bez systemowej belki (przegląd właściciela 31.10: „ramka ma być elegancka”):
//! nagłówek z tarczą, „?” i ✕, plakietki, tytuł, wyróżnione „Co dokładnie”, siatka szczegółów,
//! podpowiedź i stopka z przyciskami. Ten sam układ rysuje okno i podgląd.

#![allow(unsafe_code)]

use platform_contract::{PixelRect, SurfaceView};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{
    DRAW_TEXT_FORMAT, DT_CALCRECT, DT_EDITCONTROL, DT_NOPREFIX, DT_SINGLELINE, DT_WORDBREAK,
    DrawTextW, HDC, HFONT, SelectObject,
};

use super::paint::Fonts;

/// Format tekstu wielowierszowego (ten sam przy mierzeniu i rysowaniu).
pub(super) const WRAP: DRAW_TEXT_FORMAT =
    DRAW_TEXT_FORMAT(DT_WORDBREAK.0 | DT_EDITCONTROL.0 | DT_NOPREFIX.0);

/// Identyfikator przycisku „?” (`IDHELP`; poniżej przycisków decyzji — nigdy nie jest decyzją).
pub(super) const HELP_ID: u16 = 9;
/// Identyfikator ✕ w nagłówku (`IDCANCEL` — to samo co Esc: odmowa).
pub(super) const CLOSE_ID: u16 = 2;
/// Etykieta wyróżnionego pierwszego wiersza szczegółów.
pub(super) const LEAD_LABEL: &str = "Co dokładnie";

/// Szerokość okna przy 96 DPI.
const WIDTH: i32 = 600;
/// Wysokość nagłówka (pole przeciągania okna).
const HEADER: i32 = 48;
/// Margines treści z lewej i prawej.
const PAD: i32 = 24;
/// Kolumna etykiet siatki.
const LABEL_WIDTH: i32 = 128;
/// Wysokość przycisku decyzji (bez obwódki fokusu).
const BUTTON: i32 = 36;
/// Miejsce na obwódkę fokusu wokół przycisku (kontrolka jest o tyle większa z każdej strony).
pub(super) const RING: i32 = 3;
/// Najwyższa wartość jednego wiersza szczegółów (w liniach) — dłuższe są przycinane.
const MAX_VALUE_LINES: i32 = 8;

/// Piksele przy danym DPI.
pub(super) fn px(dpi: u32, v: i32) -> i32 {
    v * i32::try_from(dpi.clamp(96, 960)).unwrap_or(96) / 96
}

/// Wiersz siatki: etykieta, wartość i numer szczegółu w widoku.
pub(super) struct Row {
    pub label: RECT,
    pub value: RECT,
    pub detail: usize,
}

/// Plakietka nad tytułem (pierwsza — ryzyko, w jego kolorze i z ikoną).
pub(super) struct Chip {
    pub rect: RECT,
    pub text: String,
    pub first: bool,
}

/// Wyróżniony pierwszy szczegół („Co dokładnie”) w osobnym polu.
pub(super) struct Lead {
    pub panel: RECT,
    pub label: RECT,
    pub value: RECT,
}

/// Zmierzony układ (piksele okna = obszar klienta; okno nie ma ramki systemowej).
pub(super) struct Measured {
    pub client: (i32, i32),
    pub header: RECT,
    pub brand: RECT,
    pub help_button: Option<RECT>,
    pub close_button: RECT,
    pub chips: Vec<Chip>,
    pub title: RECT,
    pub help: Option<RECT>,
    pub lead: Option<Lead>,
    pub rows: Vec<Row>,
    pub status: RECT,
    pub footer: RECT,
    /// Kontrolki przycisków decyzji (z miejscem na obwódkę fokusu).
    pub buttons: Vec<(u16, RECT)>,
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> RECT {
    RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
}

/// Wysokość tekstu w szerokości `width` (czcionka wybierana na czas pomiaru).
pub(super) fn text_height(hdc: HDC, font: HFONT, text: &str, width: i32) -> i32 {
    text_box(hdc, font, text, width, WRAP).1
}

/// (szerokość, wysokość) tekstu przy formacie `format`.
pub(super) fn text_box(
    hdc: HDC,
    font: HFONT,
    text: &str,
    width: i32,
    format: DRAW_TEXT_FORMAT,
) -> (i32, i32) {
    let mut buf: Vec<u16> = text.encode_utf16().collect();
    if buf.is_empty() {
        buf.push(u16::from(b' '));
    }
    let mut r = rect(0, 0, width.max(1), 0);
    // SAFETY: kontekst urządzenia tego wątku; czcionka przywracana przed powrotem.
    unsafe {
        let old = SelectObject(hdc, font.into());
        let _ = DrawTextW(hdc, &mut buf, &raw mut r, format | DT_CALCRECT);
        SelectObject(hdc, old);
    }
    (r.right - r.left, r.bottom - r.top)
}

/// Plakietki z `badge` (oddzielone „ · ”), łamane do kolejnego wiersza; zwraca dół ostatniej.
fn chips(hdc: HDC, fonts: &Fonts, badge: &str, x: i32, y: i32, inner: i32) -> (Vec<Chip>, i32) {
    let s = |v: i32| px(fonts.dpi, v);
    let (h, mut cx, mut cy) = (s(24), x, y);
    let mut out: Vec<Chip> = Vec::new();
    for (i, part) in badge
        .split(" · ")
        .filter(|p| !p.trim().is_empty())
        .enumerate()
    {
        let tw = text_box(hdc, fonts.chip, part, inner, DT_SINGLELINE).0;
        let icon = if i == 0 { s(12) + s(6) } else { 0 };
        let w = (tw + icon + 2 * s(10)).min(inner);
        if cx > x && cx + w > x + inner {
            cx = x;
            cy += h + s(6);
        }
        out.push(Chip {
            rect: rect(cx, cy, w, h),
            text: part.trim().to_owned(),
            first: i == 0,
        });
        cx += w + s(6);
    }
    let bottom = out.last().map_or(y, |c| c.rect.bottom);
    (out, bottom)
}

/// Przyciski decyzji w stopce: bezpieczny (fokus startowy — „Odmów”) przy lewej krawędzi,
/// pozostałe przy prawej w kolejności widoku; gdy się nie mieszczą — jeden pod drugim na całą
/// szerokość (napis nigdy nie jest ucięty). Prostokąty kontrolek z miejscem na obwódkę.
fn buttons(
    hdc: HDC,
    fonts: &Fonts,
    view: &SurfaceView,
    x: i32,
    y: i32,
    inner: i32,
) -> Vec<(u16, RECT)> {
    let s = |v: i32| px(fonts.dpi, v);
    let (gap, bh, ring) = (s(8), s(BUTTON), s(RING));
    let widths: Vec<i32> = view
        .buttons
        .iter()
        .map(|b| {
            (text_box(hdc, fonts.button, &b.label, inner, DT_SINGLELINE).0 + 2 * s(16)).max(s(96))
        })
        .collect();
    let n = i32::try_from(widths.len()).unwrap_or(1).max(1);
    let grow = |r: RECT| RECT {
        left: r.left - ring,
        top: r.top - ring,
        right: r.right + ring,
        bottom: r.bottom + ring,
    };
    if widths.iter().sum::<i32>() + (n - 1) * gap + s(24) > inner {
        return view
            .buttons
            .iter()
            .zip(0..)
            .map(|(b, i)| (b.id, grow(rect(x, y + i * (bh + gap), inner, bh))))
            .collect();
    }
    let mut right = x + inner;
    let mut out: Vec<(u16, RECT)> = Vec::new();
    for (b, w) in view.buttons.iter().zip(&widths).rev() {
        let left = if b.id == view.initial_focus {
            x
        } else {
            right - w
        };
        if b.id != view.initial_focus {
            right -= w + gap;
        }
        out.push((b.id, grow(rect(left, y, *w, bh))));
    }
    out.reverse();
    out
}

/// Układa okno dla widoku (opis w nagłówku modułu).
pub(super) fn measure(
    hdc: HDC,
    fonts: &Fonts,
    view: &SurfaceView,
    help_open: bool,
    max_height: i32,
) -> Measured {
    let s = |v: i32| px(fonts.dpi, v);
    let w = s(WIDTH);
    let (x, inner) = (s(PAD), w - 2 * s(PAD));
    let header = rect(0, 0, w, s(HEADER));
    let icon = s(36);
    let close_button = rect(w - s(8) - icon, s(6), icon, icon);
    let help_button =
        (!view.help.is_empty()).then(|| rect(close_button.left - s(2) - icon, s(6), icon, icon));
    let brand_right = help_button.map_or(close_button.left, |r| r.left) - s(8);
    let brand = rect(s(20), 0, brand_right - s(20), s(HEADER));
    let (chips, mut y) = chips(hdc, fonts, &view.badge, x, s(HEADER + 4), inner);
    y += if chips.is_empty() { 0 } else { s(14) };
    let title = rect(
        x,
        y,
        inner,
        text_height(hdc, fonts.title, &view.title, inner),
    );
    y = title.bottom + s(16);
    let help = (help_open && !view.help.is_empty()).then(|| {
        let h = text_height(hdc, fonts.body, &view.help, inner - s(30 + 28)) + s(28);
        let r = rect(x, y, inner, h);
        y += h + s(16);
        r
    });
    let lead = view
        .details
        .first()
        .filter(|(k, _)| k == LEAD_LABEL)
        .map(|(k, v)| {
            let (lw, vw) = (inner - s(32), inner - s(32));
            let lh = text_height(hdc, fonts.lead_label, k, lw);
            let line = text_height(hdc, fonts.lead, "Ag", vw);
            let vh = text_height(hdc, fonts.lead, v, vw).min(line * MAX_VALUE_LINES);
            let label = rect(x + s(16), y + s(12), lw, lh);
            let value = rect(x + s(16), label.bottom + s(4), vw, vh);
            let panel = rect(x, y, inner, value.bottom + s(13) - y);
            y = panel.bottom + s(16);
            Lead {
                panel,
                label,
                value,
            }
        });
    let label_w = s(LABEL_WIDTH);
    let value_x = x + label_w + s(12);
    let value_w = inner - label_w - s(12);
    let line = text_height(hdc, fonts.body, "Ag", value_w);
    let skip = usize::from(lead.is_some());
    let rows: Vec<Row> = view
        .details
        .iter()
        .enumerate()
        .skip(skip)
        .map(|(i, (k, v))| {
            let vh = text_height(hdc, fonts.body, v, value_w).min(line * MAX_VALUE_LINES);
            let h = vh.max(text_height(hdc, fonts.label, k, label_w));
            let row = Row {
                label: rect(x, y + s(1), label_w, h),
                value: rect(value_x, y, value_w, h),
                detail: i,
            };
            y += h + s(10);
            row
        })
        .collect();
    y += if rows.is_empty() { 0 } else { s(6) };
    let status = rect(
        x,
        y,
        inner,
        text_height(hdc, fonts.hint, &view.status, inner),
    );
    let top = status.bottom + s(20);
    let buttons = buttons(hdc, fonts, view, x, top + s(16), inner);
    let bottom = buttons.last().map_or(top, |(_, r)| r.bottom - s(RING)) + s(16);
    let mut m = Measured {
        client: (w, bottom),
        header,
        brand,
        help_button,
        close_button,
        chips,
        title,
        help,
        lead,
        rows,
        status,
        footer: rect(0, top, w, bottom - top),
        buttons,
    };
    m.fit(max_height.max(s(320)), s(10));
    m
}

impl Measured {
    /// Za wysokie okno (mały ekran, otwarta pomoc): najpierw skraca się opis pod „?”, potem
    /// odpadają od dołu wiersze szczegółów; stopka z przyciskami zostaje zawsze widoczna.
    fn fit(&mut self, limit: i32, gap: i32) {
        let mut over = self.client.1 - limit;
        if over <= 0 {
            return;
        }
        if let Some(help) = self.help.as_mut() {
            let cut = over.min((help.bottom - help.top) - 6 * gap);
            if cut > 0 {
                help.bottom -= cut;
                self.shift(cut, true);
                over -= cut;
            }
        }
        if over <= 0 {
            return;
        }
        self.shift(over, false);
        let floor = self.status.top - gap;
        self.rows.retain(|r| r.value.bottom <= floor);
    }

    /// Przesuwa w górę stopkę i podpowiedź, a gdy `body` — także „Co dokładnie” i siatkę.
    fn shift(&mut self, by: i32, body: bool) {
        let up = |r: &mut RECT| {
            r.top -= by;
            r.bottom -= by;
        };
        self.client.1 -= by;
        up(&mut self.status);
        up(&mut self.footer);
        self.buttons.iter_mut().for_each(|(_, r)| up(r));
        if body {
            if let Some(l) = self.lead.as_mut() {
                up(&mut l.panel);
                up(&mut l.label);
                up(&mut l.value);
            }
            for row in &mut self.rows {
                up(&mut row.label);
                up(&mut row.value);
            }
        }
    }

    /// Wszystkie kontrolki: decyzje, „?” i ✕ (identyfikator, prostokąt).
    pub(super) fn controls(&self) -> Vec<(u16, RECT)> {
        let mut all = self.buttons.clone();
        all.extend(self.help_button.map(|r| (HELP_ID, r)));
        all.push((CLOSE_ID, self.close_button));
        all
    }

    /// Prostokąt jako `PixelRect` (x, y, szerokość, wysokość).
    pub(super) fn pixel(r: &RECT) -> PixelRect {
        (r.left, r.top, r.right - r.left, r.bottom - r.top)
    }
}
