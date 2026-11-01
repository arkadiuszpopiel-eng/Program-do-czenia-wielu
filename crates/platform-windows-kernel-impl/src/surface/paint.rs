//! Wygląd okna Sędziego Cieni w systemie wyglądu Alfy (`packages/ui-kit`): okno bez systemowej
//! belki, z cieniem i zaokrąglonymi rogami DWM i cienką linią w kolorze ryzyka u góry; nagłówek
//! z tarczą, „?” i ✕ (ikony Segoe Fluent Icons), plakietki-pigułki, tytuł Segoe UI Variable
//! Display, wyróżnione pole „Co dokładnie”, siatka „etykieta / wartość” i stopka z przyciskami
//! (główny wypełniony, pozostałe obrysowane; najechanie, wciśnięcie, obwódka fokusu). Kształty
//! wygładzane przez GDI+ (`shapes`). Tylko wygląd: wejście i decyzje zostają w `win`/`input`.

#![allow(unsafe_code)]

use std::ffi::c_void;

use platform_contract::{SurfaceTone, SurfaceView};
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
    DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DeleteObject, FW_NORMAL,
    FW_SEMIBOLD, HDC, HFONT, OUT_DEFAULT_PRECIS,
};
use windows::core::{PCWSTR, w};

use super::layout::{LEAD_LABEL, Measured, WRAP, WRAP_END, px};
use super::shapes::{CENTER, LINE, fill, glyph, mix, round_rect, text};
use super::theme::{Palette, palette};

/// Nazwa okna w nagłówku i na pasku zadań (właściciel 31.10).
pub(super) const WINDOW_NAME: &str = "Sędzia Cieni";

/// `DRAWITEMSTRUCT` (winuser.h) — bez feature `Win32_UI_Controls`; układ jak w dokumentacji Windows.
#[repr(C)]
pub(super) struct DrawItem {
    ctl_type: u32,
    pub ctl_id: u32,
    item_id: u32,
    item_action: u32,
    pub item_state: u32,
    pub hwnd_item: HWND,
    pub hdc: HDC,
    pub rc_item: RECT,
    item_data: usize,
}

/// `ODS_SELECTED` — przycisk wciśnięty.
pub(super) const ODS_SELECTED: u32 = 0x0001;
/// `ODS_FOCUS` — przycisk ma fokus klawiatury.
pub(super) const ODS_FOCUS: u32 = 0x0010;

fn font(px: i32, weight: u32, face: PCWSTR) -> HFONT {
    let weight = i32::try_from(weight).unwrap_or(400);
    let (charset, out, clip) = (DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS);
    // SAFETY: czcionka systemowa (bez fontów webowych); zwalniana w `Fonts::drop`.
    unsafe {
        CreateFontW(
            -px,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            charset,
            out,
            clip,
            CLEARTYPE_QUALITY,
            0,
            face,
        )
    }
}

/// Czcionki w rozmiarach tokenów (`--alfa-font-size-*`) dla danego DPI.
pub(super) struct Fonts {
    pub dpi: u32,
    /// Tytuł: 20 px, półgruby, krój „Display”.
    pub title: HFONT,
    /// Nazwa „Sędzia Cieni” w nagłówku i plakietki: 12 px, półgruby.
    pub chip: HFONT,
    /// Treść: 14 px.
    pub body: HFONT,
    /// Etykieta pola „Co dokładnie”: 12 px, półgruby.
    pub lead_label: HFONT,
    /// Wartość pola „Co dokładnie”: 15 px, półgruby.
    pub lead: HFONT,
    /// Etykiety siatki: 13 px.
    pub label: HFONT,
    /// Podpowiedź: 12 px.
    pub hint: HFONT,
    /// Przyciski: 14 px, półgruby.
    pub button: HFONT,
    /// Ikony 16 px (Segoe Fluent Icons).
    pub icon: HFONT,
    /// Ikony 12 px (plakietka ryzyka).
    pub icon_small: HFONT,
}

impl Fonts {
    pub(super) fn new(dpi: u32) -> Self {
        let s = |v: i32| px(dpi, v);
        let (text, display, icons) = (
            w!("Segoe UI Variable Text"),
            w!("Segoe UI Variable Display"),
            w!("Segoe Fluent Icons"),
        );
        let (semi, normal) = (FW_SEMIBOLD.0, FW_NORMAL.0);
        Self {
            dpi,
            title: font(s(20), semi, display),
            chip: font(s(12), semi, text),
            body: font(s(14), normal, text),
            lead_label: font(s(12), semi, text),
            lead: font(s(15), semi, text),
            label: font(s(13), normal, text),
            hint: font(s(12), normal, text),
            button: font(s(14), semi, text),
            icon: font(s(16), normal, icons),
            icon_small: font(s(12), normal, icons),
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        let all = [
            self.title,
            self.chip,
            self.body,
            self.lead_label,
            self.lead,
            self.label,
            self.hint,
            self.button,
            self.icon,
            self.icon_small,
        ];
        for f in all {
            // SAFETY: czcionki tego wątku, niewybrane już w żadnym kontekście.
            let _ = unsafe { DeleteObject(f.into()) };
        }
    }
}

/// Wygląd okna: motyw, czcionki dla DPI, kolor ryzyka.
pub(super) struct Look {
    pub palette: Palette,
    pub fonts: Fonts,
    pub tone: SurfaceTone,
}

/// Prostokąt pomniejszony o `by` z każdej strony.
pub(super) fn inset(r: RECT, by: i32) -> RECT {
    RECT {
        left: r.left + by,
        top: r.top + by,
        right: r.right - by,
        bottom: r.bottom - by,
    }
}

pub(super) const fn rgb(v: u32) -> COLORREF {
    COLORREF(((v >> 16) & 0xFF) | (v & 0xFF00) | ((v & 0xFF) << 16))
}

impl Look {
    pub(super) fn new(dpi: u32, palette: Palette) -> Self {
        Self {
            palette,
            fonts: Fonts::new(dpi),
            tone: SurfaceTone::Neutral,
        }
    }

    /// Motyw systemu (mógł się zmienić) i czcionki dla DPI.
    pub(super) fn refresh(&mut self, dpi: u32) {
        self.palette = palette();
        if self.fonts.dpi != dpi {
            self.fonts = Fonts::new(dpi);
        }
    }

    pub(super) fn s(&self, v: i32) -> i32 {
        px(self.fonts.dpi, v)
    }

    pub(super) fn danger(&self) -> bool {
        matches!(self.tone, SurfaceTone::High | SurfaceTone::Critical)
    }

    /// Tło stopki z przyciskami (o ton od tła okna).
    pub(super) fn footer(&self) -> COLORREF {
        let p = &self.palette;
        if p.dark {
            p.bg
        } else {
            mix(p.surface2, p.surface, 55)
        }
    }

    /// Ciemny motyw ramki, zaokrąglone rogi i kolor krawędzi okna (DWM); błąd (starszy system)
    /// pomijamy — okno działa dalej, tylko bez tych ozdób.
    pub(super) fn frame(&self, hwnd: HWND) {
        let p = &self.palette;
        let dark = i32::from(p.dark);
        let round = DWMWCP_ROUND;
        let edge = if p.dark {
            mix(p.text, p.surface, 16)
        } else {
            mix(p.text, p.surface, 18)
        };
        // SAFETY: atrybuty DWM naszego okna z wartości lokalnych.
        unsafe {
            let set = |attr, ptr: *const c_void| DwmSetWindowAttribute(hwnd, attr, ptr, 4);
            let _ = set(DWMWA_USE_IMMERSIVE_DARK_MODE, (&raw const dark).cast());
            let _ = set(DWMWA_WINDOW_CORNER_PREFERENCE, (&raw const round).cast());
            let _ = set(DWMWA_BORDER_COLOR, (&raw const edge).cast());
        }
    }

    /// Całe okno poza przyciskami.
    pub(super) fn paint(&self, hdc: HDC, view: &SurfaceView, m: &Measured) {
        let (p, f) = (&self.palette, &self.fonts);
        let s = |v: i32| self.s(v);
        let risk = p.risk(self.tone);
        let all = RECT {
            left: 0,
            top: 0,
            right: m.client.0,
            bottom: m.client.1,
        };
        fill(hdc, &all, p.surface);
        fill(
            hdc,
            &RECT {
                bottom: s(3),
                ..all
            },
            risk,
        );
        let shield = RECT {
            right: m.brand.left + s(18),
            ..m.brand
        };
        text(hdc, f.icon, risk, glyph::SHIELD, shield, CENTER);
        let name = RECT {
            left: m.brand.left + s(26),
            ..m.brand
        };
        text(hdc, f.chip, p.muted, WINDOW_NAME, name, LINE);
        for chip in &m.chips {
            self.chip(hdc, chip.rect, &chip.text, chip.first);
        }
        text(hdc, f.title, p.text, &view.title, m.title, WRAP);
        if let Some(h) = m.help {
            round_rect(hdc, &h, s(8), Some(p.surface2), None);
            let icon = RECT {
                left: h.left + s(14),
                top: h.top + s(13),
                right: h.left + s(34),
                bottom: h.top + s(33),
            };
            text(hdc, f.icon, p.focus, glyph::INFO, icon, CENTER);
            let body = RECT {
                left: h.left + s(44),
                top: h.top + s(14),
                right: h.right - s(14),
                bottom: h.bottom - s(14),
            };
            text(hdc, f.body, p.text, &view.help, body, WRAP_END);
        }
        if let Some(l) = &m.lead {
            let tint = if p.dark { 10 } else { 6 };
            let edge = if p.dark { 34 } else { 24 };
            round_rect(
                hdc,
                &l.panel,
                s(8),
                Some(mix(risk, p.surface, tint)),
                Some((mix(risk, p.surface, edge), s(1))),
            );
            text(hdc, f.lead_label, p.muted, LEAD_LABEL, l.label, WRAP);
            let value = view.details.first().map_or("", |(_, v)| v.as_str());
            text(hdc, f.lead, p.text, value, l.value, WRAP);
        }
        for row in &m.rows {
            if let Some((k, v)) = view.details.get(row.detail) {
                text(hdc, f.label, p.muted, k, row.label, WRAP);
                text(hdc, f.body, p.text, v, row.value, WRAP);
            }
        }
        text(hdc, f.hint, p.subtle, &view.status, m.status, WRAP);
        fill(hdc, &m.footer, self.footer());
        fill(
            hdc,
            &RECT {
                bottom: m.footer.top + s(1).max(1),
                ..m.footer
            },
            p.border,
        );
        if let (Some(r), Some(c)) = (m.choice_label, &view.choice) {
            text(hdc, f.label, p.muted, &c.label, r, LINE);
        }
    }

    /// Plakietka-pigułka: pierwsza w kolorze ryzyka z ikoną, pozostałe stonowane.
    fn chip(&self, hdc: HDC, r: RECT, label: &str, first: bool) {
        let (p, f) = (&self.palette, &self.fonts);
        let s = |v: i32| self.s(v);
        let h = r.bottom - r.top;
        if !first {
            round_rect(hdc, &r, h / 2, Some(p.surface2), None);
            text(hdc, f.chip, p.muted, label, inset(r, 0), CENTER);
            return;
        }
        let risk = p.risk(self.tone);
        let tint = if p.dark { 18 } else { 11 };
        round_rect(hdc, &r, h / 2, Some(mix(risk, p.surface, tint)), None);
        let icon = match self.tone {
            SurfaceTone::Neutral => glyph::INFO,
            SurfaceTone::Low => glyph::OK,
            SurfaceTone::Medium | SurfaceTone::High => glyph::WARNING,
            SurfaceTone::Critical => glyph::STOP,
        };
        let ir = RECT {
            left: r.left + s(10),
            right: r.left + s(22),
            ..r
        };
        text(hdc, f.icon_small, risk, icon, ir, CENTER);
        let tr = RECT {
            left: r.left + s(28),
            right: r.right - s(10),
            ..r
        };
        text(hdc, f.chip, risk, label, tr, LINE);
    }
}
