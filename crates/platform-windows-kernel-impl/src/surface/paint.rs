//! Wygląd okna Sędziego Cieni w systemie wyglądu Alfy (`packages/ui-kit`, karta `ApprovalCard`):
//! tło okna i karta z obramowaniem i paskiem ryzyka po lewej, nagłówek „Sędzia Cieni” z „?”
//! w kółku, plakietka ryzyka na tle w kolorze ryzyka, siatka „etykieta / wartość” (pierwszy wiersz
//! „Co dokładnie” pogrubiony), kolory z tokenów `--alfa-color-*` w motywie jasnym i ciemnym,
//! Segoe UI Variable skalowane z DPI, przyciski jak `Button` z ramką fokusu. Cały tekst rysowany
//! tutaj (bez kontrolek STATIC), w układzie zmierzonym przez `layout`. Tylko wygląd: wejście
//! i decyzje zostają w `win`/`input`.

#![allow(unsafe_code)]

use std::ffi::c_void;

use platform_contract::{SurfaceTone, SurfaceView};
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreatePen, CreateSolidBrush,
    DEFAULT_CHARSET, DRAW_TEXT_FORMAT, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, Ellipse, FW_NORMAL, FW_SEMIBOLD, FillRect, GetStockObject, HDC, HFONT, NULL_BRUSH,
    OUT_DEFAULT_PRECIS, PS_SOLID, RoundRect, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::core::w;

use super::layout::{Measured, WRAP};
use super::theme::{Palette, palette};

/// Nazwa okna w nagłówku i na pasku tytułu (właściciel 31.10).
pub(super) const WINDOW_NAME: &str = "Sędzia Cieni";

/// `DRAWITEMSTRUCT` (winuser.h) — bez feature `Win32_UI_Controls`; układ jak w dokumentacji Windows.
#[repr(C)]
pub(super) struct DrawItem {
    ctl_type: u32,
    pub ctl_id: u32,
    item_id: u32,
    item_action: u32,
    pub item_state: u32,
    hwnd_item: HWND,
    pub hdc: HDC,
    pub rc_item: RECT,
    item_data: usize,
}

/// `ODS_SELECTED` — przycisk wciśnięty.
pub(super) const ODS_SELECTED: u32 = 0x0001;
/// `ODS_FOCUS` — przycisk ma fokus klawiatury.
pub(super) const ODS_FOCUS: u32 = 0x0010;

fn fill(hdc: HDC, rect: &RECT, color: COLORREF) {
    // SAFETY: pędzel GDI tego wątku, zwalniany zaraz po wypełnieniu.
    unsafe {
        let b = CreateSolidBrush(color);
        FillRect(hdc, rect, b);
        let _ = DeleteObject(b.into());
    }
}

/// Zaokrąglony prostokąt albo koło (`radius = 0` → `Ellipse`): wypełnienie i obrys.
fn shape(hdc: HDC, r: RECT, radius: Option<i32>, body: Option<COLORREF>, line: COLORREF, w: i32) {
    // SAFETY: obiekty GDI wybrane na czas rysowania, przywrócone i zwolnione przed powrotem.
    unsafe {
        let pen = CreatePen(PS_SOLID, w, line);
        let brush = body.map(|c| CreateSolidBrush(c));
        let old_pen = SelectObject(hdc, pen.into());
        let old_brush = match brush {
            Some(b) => SelectObject(hdc, b.into()),
            None => SelectObject(hdc, GetStockObject(NULL_BRUSH)),
        };
        let _ = match radius {
            Some(rad) => RoundRect(hdc, r.left, r.top, r.right, r.bottom, rad, rad),
            None => Ellipse(hdc, r.left, r.top, r.right, r.bottom),
        };
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(pen.into());
        if let Some(b) = brush {
            let _ = DeleteObject(b.into());
        }
    }
}

/// Tekst w prostokącie (format jak przy pomiarze).
fn text(hdc: HDC, font: HFONT, color: COLORREF, s: &str, mut r: RECT, format: DRAW_TEXT_FORMAT) {
    let mut buf: Vec<u16> = s.encode_utf16().collect();
    if buf.is_empty() {
        return;
    }
    // SAFETY: tekst w kontekście na czas rysowania; czcionka przywracana.
    unsafe {
        let old = SelectObject(hdc, font.into());
        SetTextColor(hdc, color);
        SetBkMode(hdc, TRANSPARENT);
        let _ = DrawTextW(hdc, &mut buf, &raw mut r, format);
        SelectObject(hdc, old);
    }
}

/// Mieszanie koloru `a` z `b` (`t` z 0–100: udział `a`).
fn mix(a: COLORREF, b: COLORREF, t: u32) -> COLORREF {
    let ch = |c: COLORREF, shift: u32| (c.0 >> shift) & 0xFF;
    let m = |shift: u32| (ch(a, shift) * t + ch(b, shift) * (100 - t)) / 100;
    COLORREF(m(0) | (m(8) << 8) | (m(16) << 16))
}

fn font(px: i32, weight: u32) -> HFONT {
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
            w!("Segoe UI Variable Text"),
        )
    }
}

/// Czcionki w rozmiarach tokenów (`--alfa-font-size-*`) dla danego DPI.
pub(super) struct Fonts {
    pub dpi: u32,
    /// Tytuł: 20 px, półgruby (`xl`).
    pub title: HFONT,
    /// Nagłówek okna i plakietka: 13 px, półgruby (`sm`).
    pub badge: HFONT,
    /// Treść: 14 px (`md`).
    pub body: HFONT,
    /// Treść wyróżniona („Co dokładnie”): 14 px, półgruby.
    pub strong: HFONT,
    /// Etykiety siatki: 13 px.
    pub label: HFONT,
    /// Podpowiedź: 12 px (`xs`).
    pub hint: HFONT,
    /// Przyciski: 14 px, półgruby (`Button`).
    pub button: HFONT,
}

impl Fonts {
    pub(super) fn new(dpi: u32) -> Self {
        let dpi_i = i32::try_from(dpi.clamp(96, 960)).unwrap_or(96);
        let s = |px: i32| px * dpi_i / 96;
        Self {
            dpi,
            title: font(s(20), FW_SEMIBOLD.0),
            badge: font(s(13), FW_SEMIBOLD.0),
            body: font(s(14), FW_NORMAL.0),
            strong: font(s(14), FW_SEMIBOLD.0),
            label: font(s(13), FW_NORMAL.0),
            hint: font(s(12), FW_NORMAL.0),
            button: font(s(14), FW_SEMIBOLD.0),
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        let all = [
            self.title,
            self.badge,
            self.body,
            self.strong,
            self.label,
            self.hint,
            self.button,
        ];
        for f in all {
            // SAFETY: czcionki tego wątku, niewybrane już w żadnym kontekście.
            let _ = unsafe { DeleteObject(f.into()) };
        }
    }
}

/// Stan przycisku do narysowania.
pub(super) struct ButtonState {
    pub pressed: bool,
    pub focused: bool,
    /// Przycisk bezpieczny (odmowa) — drugorzędny.
    pub safe: bool,
    /// „?” — koło; `on` = pomoc otwarta.
    pub help: Option<bool>,
}

/// Wygląd okna: motyw, czcionki dla DPI, kolor ryzyka.
pub(super) struct Look {
    pub palette: Palette,
    pub fonts: Fonts,
    pub tone: SurfaceTone,
    scale: i32,
}

impl Look {
    pub(super) fn new(dpi: u32, palette: Palette) -> Self {
        let mut look = Self {
            palette,
            fonts: Fonts::new(dpi),
            tone: SurfaceTone::Neutral,
            scale: 1,
        };
        look.set_dpi(dpi);
        look
    }

    /// Motyw systemu (mógł się zmienić) i czcionki dla DPI.
    pub(super) fn refresh(&mut self, dpi: u32) {
        self.palette = palette();
        self.set_dpi(dpi);
    }

    fn set_dpi(&mut self, dpi: u32) {
        if self.fonts.dpi != dpi {
            self.fonts = Fonts::new(dpi);
        }
        self.scale = i32::try_from(dpi.clamp(96, 960) / 96).unwrap_or(1).max(1);
    }

    /// Ciemny pasek tytułu i zaokrąglone rogi (DWM); błąd (starszy system) pomijamy.
    pub(super) fn frame(&self, hwnd: HWND) {
        let dark = i32::from(self.palette.dark);
        let round = DWMWCP_ROUND;
        // SAFETY: atrybuty DWM naszego okna z wartości lokalnych.
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                (&raw const dark).cast::<c_void>(),
                4,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&raw const round).cast::<c_void>(),
                4,
            );
        }
    }

    /// Całe okno poza przyciskami: tło, karta, pasek ryzyka, nagłówek, tytuł, plakietka, pomoc,
    /// siatka szczegółów i podpowiedź.
    pub(super) fn paint(&self, hdc: HDC, view: &SurfaceView, m: &Measured) {
        let (p, s, f) = (&self.palette, self.scale, &self.fonts);
        let risk = p.risk(self.tone);
        let client = RECT {
            left: 0,
            top: 0,
            right: m.client.0,
            bottom: m.client.1,
        };
        fill(hdc, &client, p.bg);
        shape(hdc, m.card, Some(2 * 10 * s), Some(p.surface), p.border, s);
        let stripe = RECT {
            left: m.card.left + s,
            top: m.card.top + 10 * s,
            right: m.card.left + 5 * s,
            bottom: m.card.bottom - 10 * s,
        };
        fill(hdc, &stripe, risk);
        let header = WINDOW_NAME.to_uppercase();
        let one = DRAW_TEXT_FORMAT(DT_SINGLELINE.0 | DT_VCENTER.0);
        text(hdc, f.badge, p.muted, &header, m.header, one);
        text(hdc, f.title, p.text, &view.title, m.title, WRAP);
        shape(
            hdc,
            m.badge,
            Some(m.badge.bottom - m.badge.top),
            Some(mix(risk, p.surface, 14)),
            mix(risk, p.surface, 45),
            s,
        );
        let center = DRAW_TEXT_FORMAT(DT_SINGLELINE.0 | DT_VCENTER.0 | DT_CENTER.0);
        text(hdc, f.badge, risk, &view.badge, m.badge, center);
        if let Some(h) = m.help {
            shape(hdc, h, Some(2 * 8 * s), Some(p.surface2), p.border, s);
            let inner = RECT {
                left: h.left + 14 * s,
                top: h.top + 12 * s,
                right: h.right - 14 * s,
                bottom: h.bottom - 12 * s,
            };
            text(hdc, f.body, p.text, &view.help, inner, WRAP);
        }
        let rule = |y: i32| {
            let r = RECT {
                left: m.title.left,
                top: y,
                right: m.title.right,
                bottom: y + s,
            };
            fill(hdc, &r, p.border);
        };
        rule(m.rule_top);
        for (row, (k, v)) in m.rows.iter().zip(&view.details) {
            text(hdc, f.label, p.muted, k, row.label, WRAP);
            let font = if row.strong { f.strong } else { f.body };
            text(hdc, font, p.text, v, row.value, WRAP);
        }
        rule(m.rule_bottom);
        text(hdc, f.hint, p.subtle, &view.status, m.status, WRAP);
    }

    /// Przycisk jak `Button` z `ui-kit` (albo „?” w kółku) z ramką fokusu na zewnątrz.
    pub(super) fn button(&self, hdc: HDC, r: RECT, label: &str, st: &ButtonState) {
        let (p, s) = (&self.palette, self.scale);
        fill(hdc, &r, p.surface);
        if let Some(on) = st.help {
            if st.focused {
                shape(hdc, r, None, None, p.focus, 2 * s);
            }
            let inner = RECT {
                left: r.left + 3 * s,
                top: r.top + 3 * s,
                right: r.right - 3 * s,
                bottom: r.bottom - 3 * s,
            };
            let (body, ink) = if on {
                (p.text, p.surface)
            } else {
                (p.surface, p.muted)
            };
            shape(
                hdc,
                inner,
                None,
                Some(body),
                if on { p.text } else { p.border },
                s,
            );
            let center = DRAW_TEXT_FORMAT(DT_SINGLELINE.0 | DT_VCENTER.0 | DT_CENTER.0);
            text(hdc, self.fonts.button, ink, "?", inner, center);
            return;
        }
        let danger = matches!(self.tone, SurfaceTone::High | SurfaceTone::Critical);
        let (body, line, ink) = if st.safe {
            let body = if st.pressed { p.surface2 } else { p.surface };
            (body, p.border, p.text)
        } else if danger {
            (p.error, p.error, p.on_accent)
        } else {
            (p.text, p.text, if p.dark { p.bg } else { p.on_accent })
        };
        let inner = RECT {
            left: r.left + 3 * s,
            top: r.top + 3 * s,
            right: r.right - 3 * s,
            bottom: r.bottom - 3 * s,
        };
        if st.focused {
            shape(hdc, r, Some(2 * 9 * s), None, p.focus, 2 * s);
        }
        shape(hdc, inner, Some(2 * 6 * s), Some(body), line, s);
        let center = DRAW_TEXT_FORMAT(DT_SINGLELINE.0 | DT_VCENTER.0 | DT_CENTER.0);
        text(hdc, self.fonts.button, ink, label, inner, center);
    }
}
