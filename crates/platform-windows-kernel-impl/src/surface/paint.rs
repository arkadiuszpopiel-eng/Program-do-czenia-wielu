//! Wygląd okna Broker-UI w systemie wyglądu Alfy (`packages/ui-kit`, karta `ApprovalCard`;
//! zgłoszenie właściciela 2026-10-20): tło okna i karta z obramowaniem i paskiem ryzyka po lewej,
//! kolory z tokenów `--alfa-color-*` w motywie jasnym i ciemnym (ustawienie Windows), czcionki
//! Segoe UI Variable w rozmiarach tokenów skalowane z DPI, przyciski jak `Button` (drugorzędny dla
//! odmowy, `danger` albo główny dla zgody) z ramką fokusu `--alfa-color-focus`, zaokrąglone rogi
//! i ciemny pasek tytułu (DWM). Tylko wygląd: wejście i decyzje zostają w `win`/`input`.

#![allow(unsafe_code)]

use std::ffi::c_void;

use platform_contract::{PixelRect, SurfaceTone};
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Dwm::{
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreatePen, CreateSolidBrush,
    DEFAULT_CHARSET, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteObject, DrawTextW, FW_NORMAL,
    FW_SEMIBOLD, FillRect, GetStockObject, HBRUSH, HDC, HFONT, NULL_BRUSH, OUT_DEFAULT_PRECIS,
    PS_SOLID, RoundRect, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::core::w;

use super::theme::{Palette, palette};

/// `DRAWITEMSTRUCT` (winuser.h) — bez feature `Win32_UI_Controls`; układ jak w dokumentacji Windows.
#[repr(C)]
pub(super) struct DrawItem {
    ctl_type: u32,
    ctl_id: u32,
    item_id: u32,
    item_action: u32,
    item_state: u32,
    hwnd_item: HWND,
    hdc: HDC,
    rc_item: RECT,
    item_data: usize,
}

/// `ODS_SELECTED` — przycisk wciśnięty.
const ODS_SELECTED: u32 = 0x0001;
/// `ODS_FOCUS` — przycisk ma fokus klawiatury.
const ODS_FOCUS: u32 = 0x0010;

fn brush(color: COLORREF) -> HBRUSH {
    // SAFETY: pędzel GDI tego wątku (zwalniany przez `release`).
    unsafe { CreateSolidBrush(color) }
}

fn release(brush: HBRUSH) {
    // SAFETY: pędzel utworzony przez `brush`, niewybrany w żadnym kontekście.
    let _ = unsafe { DeleteObject(brush.into()) };
}

fn fill(hdc: HDC, rect: &RECT, color: COLORREF) {
    let b = brush(color);
    // SAFETY: kontekst od systemu na czas komunikatu; pędzel zwalniany od razu.
    unsafe {
        FillRect(hdc, rect, b);
    }
    release(b);
}

/// Zaokrąglony prostokąt: wypełnienie `body` (albo przezroczyste) i obrys `line` o grubości `width`.
fn round_rect(hdc: HDC, r: RECT, radius: i32, body: Option<COLORREF>, line: COLORREF, width: i32) {
    // SAFETY: obiekty GDI wybrane na czas rysowania, przywrócone i zwolnione przed powrotem.
    unsafe {
        let pen = CreatePen(PS_SOLID, width, line);
        let fill_brush = body.map(brush);
        let old_pen = SelectObject(hdc, pen.into());
        let old_brush = match fill_brush {
            Some(b) => SelectObject(hdc, b.into()),
            None => SelectObject(hdc, GetStockObject(NULL_BRUSH)),
        };
        let _ = RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(pen.into());
        if let Some(b) = fill_brush {
            release(b);
        }
    }
}

fn rect((x, y, w, h): PixelRect) -> RECT {
    RECT {
        left: x,
        top: y,
        right: x + w,
        bottom: y + h,
    }
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
    dpi: u32,
    /// Tytuł: 20 px, półgruby (`xl`).
    pub title: HFONT,
    /// Plakietka ryzyka: 13 px, półgruby (`sm`).
    pub badge: HFONT,
    /// Treść: 14 px (`md`).
    pub body: HFONT,
    /// Podpowiedź: 12 px (`xs`).
    pub hint: HFONT,
    /// Przyciski: 14 px, półgruby (`Button`).
    pub button: HFONT,
}

impl Fonts {
    fn new(dpi: u32) -> Self {
        let dpi_i = i32::try_from(dpi.clamp(96, 960)).unwrap_or(96);
        let s = |px: i32| px * dpi_i / 96;
        Self {
            dpi,
            title: font(s(20), FW_SEMIBOLD.0),
            badge: font(s(13), FW_SEMIBOLD.0),
            body: font(s(14), FW_NORMAL.0),
            hint: font(s(12), FW_NORMAL.0),
            button: font(s(14), FW_SEMIBOLD.0),
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        for f in [self.title, self.badge, self.body, self.hint, self.button] {
            // SAFETY: czcionki tego wątku, niewybrane już w kontrolkach (kontrolki zniszczone
            // przed wymianą zestawu albo wątek się kończy).
            let _ = unsafe { DeleteObject(f.into()) };
        }
    }
}

/// Wygląd bieżącej karty.
pub(super) struct Look {
    palette: Palette,
    surface: HBRUSH,
    fonts: Fonts,
    /// Kolor ryzyka karty.
    pub tone: SurfaceTone,
    /// Karta w oknie (piksele obszaru klienta).
    pub card: PixelRect,
    /// Skala (DPI/96, co najmniej 1) dla promieni i grubości.
    scale: i32,
    /// Plakietka (kolor ryzyka), etykiety (przygaszone) i podpowiedź (stonowana).
    pub badge: Option<HWND>,
    pub muted: Vec<HWND>,
    pub status: Option<HWND>,
    /// Przycisk bezpieczny (odmowa) — drugorzędny; pozostałe — `danger` albo główny.
    pub safe: u16,
    pub labels: Vec<(u16, Vec<u16>)>,
}

impl Look {
    pub(super) fn new() -> Self {
        let palette = palette();
        Self {
            surface: brush(palette.surface),
            palette,
            fonts: Fonts::new(96),
            tone: SurfaceTone::Neutral,
            card: (0, 0, 0, 0),
            scale: 1,
            badge: None,
            muted: Vec::new(),
            status: None,
            safe: 0,
            labels: Vec::new(),
        }
    }

    /// Przed ułożeniem karty: motyw (mógł się zmienić), czcionki dla DPI, ramka DWM. Woła się po
    /// zniszczeniu starych kontrolek (czcionki starego DPI nie są już nigdzie wybrane).
    pub(super) fn prepare(&mut self, hwnd: HWND, dpi: u32) -> &Fonts {
        let palette = palette();
        if palette.dark != self.palette.dark {
            release(self.surface);
            self.surface = brush(palette.surface);
            self.palette = palette;
        }
        if self.fonts.dpi != dpi {
            self.fonts = Fonts::new(dpi);
        }
        self.scale = i32::try_from(dpi.clamp(96, 960) / 96).unwrap_or(1).max(1);
        self.muted.clear();
        self.labels.clear();
        let dark = i32::from(self.palette.dark);
        let round = DWMWCP_ROUND;
        // SAFETY: atrybuty DWM naszego okna z wartości lokalnych; błąd (starszy system) pomijamy.
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
        &self.fonts
    }

    /// `WM_CTLCOLORSTATIC`/`WM_CTLCOLORBTN`: kolor tekstu i tło karty (wszystkie teksty leżą na karcie).
    pub(super) fn control_colors(&self, hdc: HDC, control: isize) -> HBRUSH {
        let is = |h: &HWND| h.0 as isize == control;
        let text = if self.badge.as_ref().is_some_and(is) {
            self.palette.risk(self.tone)
        } else if self.status.as_ref().is_some_and(is) {
            self.palette.subtle
        } else if self.muted.iter().any(is) {
            self.palette.muted
        } else {
            self.palette.text
        };
        // SAFETY: kontekst urządzenia od systemu na czas komunikatu.
        unsafe {
            SetTextColor(hdc, text);
            SetBkMode(hdc, TRANSPARENT);
        }
        self.surface
    }

    /// `WM_ERASEBKGND`: tło okna, karta z obramowaniem i pasek ryzyka po lewej.
    pub(super) fn erase(&self, hdc: HDC, client: RECT) {
        let p = &self.palette;
        fill(hdc, &client, p.bg);
        let s = self.scale;
        let card = rect(self.card);
        round_rect(hdc, card, 2 * 10 * s, Some(p.surface), p.border, s);
        let stripe = RECT {
            left: card.left,
            top: card.top + 6 * s,
            right: card.left + 3 * s,
            bottom: card.bottom - 6 * s,
        };
        fill(hdc, &stripe, p.risk(self.tone));
    }

    /// `WM_DRAWITEM` przycisku: wygląd `Button` z `ui-kit`.
    pub(super) fn draw(&self, dis: &DrawItem) {
        let Ok(id) = u16::try_from(dis.ctl_id) else {
            return;
        };
        let label = self
            .labels
            .iter()
            .find(|(i, _)| *i == id)
            .map_or(&[][..], |(_, l)| l.as_slice());
        let p = &self.palette;
        let pressed = (dis.item_state & ODS_SELECTED) != 0;
        let focused = (dis.item_state & ODS_FOCUS) != 0;
        let danger = matches!(self.tone, SurfaceTone::High | SurfaceTone::Critical);
        let (body, line, text) = if id == self.safe {
            let body = if pressed { p.surface2 } else { p.surface };
            (body, p.border, p.text)
        } else if danger {
            (p.error, p.error, p.on_accent)
        } else {
            (p.text, p.text, if p.dark { p.bg } else { p.on_accent })
        };
        let (hdc, r, s) = (dis.hdc, dis.rc_item, self.scale);
        fill(hdc, &r, p.surface);
        // Ramka fokusu na zewnątrz przycisku (`outline: 2px focus`, odstęp 1 px).
        let inner = RECT {
            left: r.left + 3 * s,
            top: r.top + 3 * s,
            right: r.right - 3 * s,
            bottom: r.bottom - 3 * s,
        };
        if focused {
            round_rect(hdc, r, 2 * 9 * s, None, p.focus, 2 * s);
        }
        round_rect(hdc, inner, 2 * 6 * s, Some(body), line, s);
        // SAFETY: tekst w kontekście przycisku na czas WM_DRAWITEM; czcionka przywracana.
        unsafe {
            let old = SelectObject(hdc, self.fonts.button.into());
            SetTextColor(hdc, text);
            SetBkMode(hdc, TRANSPARENT);
            let mut area = inner;
            let mut buf = label.to_vec();
            let _ = DrawTextW(
                hdc,
                &mut buf,
                &raw mut area,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            SelectObject(hdc, old);
        }
    }
}

impl Drop for Look {
    fn drop(&mut self) {
        release(self.surface);
    }
}
