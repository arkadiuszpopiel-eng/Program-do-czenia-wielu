//! Przyciski okna Sędziego Cieni: decyzje w stopce (główny wypełniony — na czerwono przy wysokim
//! i krytycznym ryzyku — pozostałe obrysowane) oraz „?” i ✕ w nagłówku jako przyciski-ikony bez
//! tła (tło dopiero pod kursorem; ✕ wtedy czerwony jak w Windows). Stany: najechanie,
//! wciśnięcie, obwódka fokusu z odstępem. Kształty wygładzane przez GDI+ (`shapes`).

use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::HDC;

use super::layout::RING;
use super::paint::{Look, inset, rgb};
use super::shapes::{CENTER, fill, glyph, mix, round_rect, text};

/// Rodzaj przycisku.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// Bezpieczny („Odmów”) i pozostałe drugorzędne — obrysowane.
    Secondary,
    /// Główny („Zezwól tylko teraz”, „Włącz: …”) — wypełniony.
    Primary,
    /// Przycisk rzędu wyboru (czas zgody) — obrysowany jak drugorzędny.
    Choice,
    /// „?” w nagłówku; `true` = opis otwarty.
    Help(bool),
    /// ✕ w nagłówku (odmowa).
    Close,
}

/// Stan przycisku do narysowania.
pub(super) struct ButtonState {
    pub pressed: bool,
    pub focused: bool,
    pub hover: bool,
    pub kind: Kind,
}

impl Look {
    /// Kolory przycisku: (tło, obrys, napis).
    fn colors(&self, st: &ButtonState) -> (COLORREF, Option<COLORREF>, COLORREF) {
        let p = &self.palette;
        let step = |base: COLORREF, toward: COLORREF| {
            if st.pressed {
                mix(toward, base, 22)
            } else if st.hover {
                mix(toward, base, 11)
            } else {
                base
            }
        };
        match st.kind {
            Kind::Primary if self.danger() => {
                let base = if p.dark { rgb(0xDC2626) } else { rgb(0xB91C1C) };
                (step(base, rgb(0x000000)), None, rgb(0xFFFFFF))
            }
            Kind::Primary => {
                let ink = if p.dark { p.bg } else { rgb(0xFFFFFF) };
                let toward = if p.dark { rgb(0x000000) } else { rgb(0xFFFFFF) };
                (step(p.text, toward), None, ink)
            }
            Kind::Secondary | Kind::Choice => {
                let base = if p.dark { p.surface2 } else { p.surface };
                let edge = mix(p.text, base, if p.dark { 18 } else { 16 });
                (step(base, p.text), Some(edge), p.text)
            }
            Kind::Close if st.hover || st.pressed => {
                let base = rgb(0xC42B1C);
                (step(base, rgb(0x000000)), None, rgb(0xFFFFFF))
            }
            Kind::Help(open) => {
                let base = if open { p.surface2 } else { p.surface };
                (
                    step(base, p.text),
                    None,
                    if open { p.text } else { p.muted },
                )
            }
            Kind::Close => (p.surface, None, p.muted),
        }
    }

    /// Przycisk w prostokącie kontrolki `r` (z miejscem na obwódkę fokusu).
    pub(super) fn button(&self, hdc: HDC, r: RECT, label: &str, st: &ButtonState) {
        let p = &self.palette;
        let ring = self.s(RING);
        let icon = matches!(st.kind, Kind::Help(_) | Kind::Close);
        fill(hdc, &r, if icon { p.surface } else { self.footer() });
        let body = inset(r, ring);
        let radius = self.s(6);
        let (back, edge, ink) = self.colors(st);
        let quiet = icon && back == p.surface;
        if !quiet {
            round_rect(
                hdc,
                &body,
                radius,
                Some(back),
                edge.map(|c| (c, self.s(1).max(1))),
            );
        }
        if st.focused {
            round_rect(hdc, &r, radius + ring, None, Some((p.focus, self.s(2))));
        }
        let (font, label) = match st.kind {
            Kind::Help(_) => (self.fonts.icon, glyph::HELP),
            Kind::Close => (self.fonts.icon, glyph::CLOSE),
            _ => (self.fonts.button, label),
        };
        text(hdc, font, ink, label, body, CENTER);
    }
}
