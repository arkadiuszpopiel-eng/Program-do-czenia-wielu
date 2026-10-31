//! Stopka okna Sędziego Cieni: rząd główny („Odmów” przy lewej krawędzi, pozostałe przy prawej)
//! i — gdy widok go ma — rząd wyboru pod spodem: etykieta („Zawsze w tym zakresie:”) i mniejsze
//! przyciski czasu zgody (1 h · 3 h · 24 h · Do odwołania; właściciel 31.10). Gdy przyciski się nie
//! mieszczą — jeden pod drugim (rząd główny) albo etykieta nad przyciskami (rząd wyboru); napis
//! nigdy nie jest ucięty. Prostokąty kontrolek z miejscem na obwódkę fokusu.

use platform_contract::{SurfaceButton, SurfaceView};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{DT_SINGLELINE, HDC};

use super::layout::{RING, px, rect, text_box};
use super::paint::Fonts;

/// Wysokość przycisku rzędu głównego (bez obwódki fokusu).
const BUTTON: i32 = 36;
/// Wysokość przycisku rzędu wyboru.
const CHOICE: i32 = 32;

/// Zmierzona stopka.
pub(super) struct Footer {
    pub buttons: Vec<(u16, RECT)>,
    pub choice_label: Option<RECT>,
    /// Dół ostatniego przycisku (bez obwódki).
    pub bottom: i32,
}

fn grow(r: RECT, ring: i32) -> RECT {
    RECT {
        left: r.left - ring,
        top: r.top - ring,
        right: r.right + ring,
        bottom: r.bottom + ring,
    }
}

fn widths(hdc: HDC, fonts: &Fonts, row: &[&SurfaceButton], pad: i32, min: i32) -> Vec<i32> {
    row.iter()
        .map(|b| (text_box(hdc, fonts.button, &b.label, 4000, DT_SINGLELINE).0 + 2 * pad).max(min))
        .collect()
}

/// Rząd główny: bezpieczny (`safe`) przy lewej krawędzi, pozostałe przy prawej.
fn main_row(
    hdc: HDC,
    fonts: &Fonts,
    row: &[&SurfaceButton],
    safe: u16,
    (x, y, inner): (i32, i32, i32),
) -> Vec<(u16, RECT)> {
    let s = |v: i32| px(fonts.dpi, v);
    let (gap, bh, ring) = (s(8), s(BUTTON), s(RING));
    let widths = widths(hdc, fonts, row, s(16), s(96));
    let n = i32::try_from(widths.len()).unwrap_or(1).max(1);
    if widths.iter().sum::<i32>() + (n - 1) * gap + s(24) > inner {
        return row
            .iter()
            .zip(0..)
            .map(|(b, i)| (b.id, grow(rect(x, y + i * (bh + gap), inner, bh), ring)))
            .collect();
    }
    let mut right = x + inner;
    let mut out: Vec<(u16, RECT)> = Vec::new();
    for (b, w) in row.iter().zip(&widths).rev() {
        let left = if b.id == safe { x } else { right - w };
        if b.id != safe {
            right -= w + gap;
        }
        out.push((b.id, grow(rect(left, y, *w, bh), ring)));
    }
    out.reverse();
    out
}

/// Stopka od wysokości `y` w kolumnie (`x`, szerokość `inner`).
pub(super) fn footer(
    hdc: HDC,
    fonts: &Fonts,
    view: &SurfaceView,
    x: i32,
    y: i32,
    inner: i32,
) -> Footer {
    let s = |v: i32| px(fonts.dpi, v);
    let ring = s(RING);
    let picked = view
        .choice
        .as_ref()
        .map(|c| c.buttons.as_slice())
        .unwrap_or_default();
    let (row, chips): (Vec<&SurfaceButton>, Vec<&SurfaceButton>) =
        view.buttons.iter().partition(|b| !picked.contains(&b.id));
    let mut buttons = main_row(hdc, fonts, &row, view.initial_focus, (x, y, inner));
    let mut bottom = buttons.last().map_or(y, |(_, r)| r.bottom - ring);
    let Some(choice) = view.choice.as_ref().filter(|_| !chips.is_empty()) else {
        return Footer {
            buttons,
            choice_label: None,
            bottom,
        };
    };
    let (gap, ch) = (s(6), s(CHOICE));
    let widths = widths(hdc, fonts, &chips, s(14), s(56));
    let n = i32::try_from(widths.len()).unwrap_or(1);
    let total = widths.iter().sum::<i32>() + (n - 1) * gap;
    let (lw, lh) = text_box(hdc, fonts.label, &choice.label, inner, DT_SINGLELINE);
    let mut cy = bottom + s(14);
    let mut cx = x + inner - total;
    let label = if lw + s(16) + total <= inner {
        rect(x, cy, lw, ch)
    } else {
        // Etykieta nad przyciskami, przyciski od lewej (zawijane).
        let r = rect(x, cy, inner, lh);
        cy += lh + s(8);
        cx = x;
        r
    };
    for (b, w) in chips.iter().zip(&widths) {
        if cx > x && cx + w > x + inner {
            cx = x;
            cy += ch + gap;
        }
        buttons.push((b.id, grow(rect(cx, cy, *w, ch), ring)));
        cx += w + gap;
    }
    bottom = cy + ch;
    Footer {
        buttons,
        choice_label: Some(label),
        bottom,
    }
}
