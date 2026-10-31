//! Układ okna Sędziego Cieni mierzony prawdziwą czcionką (`DrawTextW` + `DT_CALCRECT`): wysokość
//! każdego tekstu wynika z łamania wierszy w danej szerokości, a długie słowa bez spacji (ścieżki,
//! adresy) łamią się w środku (`DT_EDITCONTROL`) — nic nie wychodzi poza kartę i nic się nie
//! nakłada (przegląd właściciela 31.10: „okno niedokładne”). Ten sam układ rysuje okno i podgląd.

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

/// Szerokość okna przy 96 DPI.
const WIDTH: i32 = 640;
/// Kolumna etykiet przy 96 DPI.
const LABEL_WIDTH: i32 = 150;
/// Najwyższa wartość jednego wiersza szczegółów (w liniach) — dłuższe są przycinane.
const MAX_VALUE_LINES: i32 = 8;

/// Wiersz szczegółów: etykieta, wartość, czy wyróżniony („Co dokładnie”).
pub(super) struct Row {
    pub label: RECT,
    pub value: RECT,
    pub strong: bool,
}

/// Zmierzony układ (piksele obszaru klienta).
pub(super) struct Measured {
    pub client: (i32, i32),
    pub card: RECT,
    pub header: RECT,
    pub help_button: Option<RECT>,
    pub title: RECT,
    pub badge: RECT,
    pub help: Option<RECT>,
    pub rule_top: i32,
    pub rows: Vec<Row>,
    pub rule_bottom: i32,
    pub status: RECT,
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

/// Układa okno dla widoku: karta z nagłówkiem („Sędzia Cieni” + „?”), tytuł, plakietka ryzyka,
/// pomoc (gdy otwarta), siatka „etykieta / wartość”, podpowiedź i przyciski w stopce.
pub(super) fn measure(
    hdc: HDC,
    fonts: &Fonts,
    view: &SurfaceView,
    help_open: bool,
    max_height: i32,
) -> Measured {
    let dpi = fonts.dpi;
    let dpi_i = i32::try_from(dpi.clamp(96, 960)).unwrap_or(96);
    let s = |v: i32| v * dpi_i / 96;
    let w = s(WIDTH);
    let (margin, pad, stripe) = (s(12), s(20), s(4));
    let card_w = w - 2 * margin;
    let x = margin + stripe + pad;
    let inner = card_w - stripe - 2 * pad;
    let mut y = margin + s(14);
    let header = rect(x, y, inner - s(36), s(28));
    let help_button = (!view.help.is_empty()).then(|| rect(x + inner - s(28), y, s(28), s(28)));
    y += s(28) + s(6);
    let title = rect(
        x,
        y,
        inner,
        text_height(hdc, fonts.title, &view.title, inner),
    );
    y += title.bottom - title.top + s(10);
    let (bw, bh) = text_box(hdc, fonts.badge, &view.badge, inner, DT_SINGLELINE);
    let badge = rect(x, y, (bw + s(24)).min(inner), bh + s(10));
    y += badge.bottom - badge.top + s(14);
    let help = (help_open && !view.help.is_empty()).then(|| {
        let h = text_height(hdc, fonts.body, &view.help, inner - s(28)) + s(24);
        let r = rect(x, y, inner, h);
        y += h + s(14);
        r
    });
    let rule_top = y;
    y += s(14);
    let label_w = s(LABEL_WIDTH);
    let value_x = x + label_w + s(14);
    let value_w = inner - label_w - s(14);
    let line = text_height(hdc, fonts.body, "Ag", value_w);
    let rows = view
        .details
        .iter()
        .enumerate()
        .map(|(i, (k, v))| {
            let strong = i == 0 && k == "Co dokładnie";
            let font = if strong { fonts.strong } else { fonts.body };
            let vh = text_height(hdc, font, v, value_w).min(line * MAX_VALUE_LINES);
            let lh = text_height(hdc, fonts.label, k, label_w);
            let h = vh.max(lh);
            let row = Row {
                label: rect(x, y, label_w, h),
                value: rect(value_x, y, value_w, h),
                strong,
            };
            y += h + s(10);
            row
        })
        .collect();
    let rule_bottom = y + s(2);
    y += s(16);
    let status = rect(
        x,
        y,
        inner,
        text_height(hdc, fonts.hint, &view.status, inner),
    );
    y += status.bottom - status.top + s(16);
    // Przyciski w jednym rzędzie, szerokie według napisów; gdy się nie mieszczą — jeden pod
    // drugim na całą szerokość (napis nigdy nie jest ucięty). Kolejność bez zmian: „Odmów” pierwsza.
    let gap = s(10);
    let bh = s(40);
    let widths: Vec<i32> = view
        .buttons
        .iter()
        .map(|b| text_box(hdc, fonts.button, &b.label, inner, DT_SINGLELINE).0 + s(36))
        .collect();
    let n = i32::try_from(widths.len()).unwrap_or(1).max(1);
    let used: i32 = widths.iter().sum::<i32>() + (n - 1) * gap;
    let buttons = if used <= inner {
        let extra = (inner - used) / n;
        let mut bx = x;
        view.buttons
            .iter()
            .zip(&widths)
            .map(|(b, bw)| {
                let r = rect(bx, y, bw + extra, bh);
                bx += bw + extra + gap;
                (b.id, r)
            })
            .collect::<Vec<_>>()
    } else {
        view.buttons
            .iter()
            .zip(0..)
            .map(|(b, i)| (b.id, rect(x, y + i * (bh + gap), inner, bh)))
            .collect()
    };
    y = buttons.last().map_or(y, |(_, r)| r.bottom) + pad;
    let mut m = Measured {
        client: (w, y + margin),
        card: RECT {
            left: margin,
            top: margin,
            right: margin + card_w,
            bottom: y,
        },
        header,
        help_button,
        title,
        badge,
        help,
        rule_top,
        rows,
        rule_bottom,
        status,
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
        let floor = self.rule_bottom - gap;
        self.rows.retain(|r| r.value.bottom <= floor);
    }

    /// Przesuwa w górę stopkę (podpowiedź, przyciski, dolną linię) i — gdy `body` — także
    /// górną linię i wiersze szczegółów.
    fn shift(&mut self, by: i32, body: bool) {
        let up = |r: &mut RECT| {
            r.top -= by;
            r.bottom -= by;
        };
        self.client.1 -= by;
        self.card.bottom -= by;
        self.rule_bottom -= by;
        up(&mut self.status);
        self.buttons.iter_mut().for_each(|(_, r)| up(r));
        if body {
            self.rule_top -= by;
            for row in &mut self.rows {
                up(&mut row.label);
                up(&mut row.value);
            }
        }
    }

    /// Prostokąt przycisku jako `PixelRect` (x, y, szerokość, wysokość).
    pub(super) fn pixel(r: &RECT) -> PixelRect {
        (r.left, r.top, r.right - r.left, r.bottom - r.top)
    }
}
