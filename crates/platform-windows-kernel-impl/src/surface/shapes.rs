//! Rysowanie podstawowe okna Sędziego Cieni: kształty wygładzane przez GDI+ (zaokrąglone
//! prostokąty, koła — bez poszarpanych brzegów GDI), wypełnienia, tekst GDI z ClearType i ikony
//! z systemowej czcionki Segoe Fluent Icons (Windows 11; bez fontów webowych).

#![allow(unsafe_code)]

use std::sync::OnceLock;

use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DRAW_TEXT_FORMAT, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, FillRect, HDC, HFONT, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::Graphics::GdiPlus::{
    FillModeAlternate, GdipAddPathArc, GdipClosePathFigure, GdipCreateFromHDC, GdipCreatePath,
    GdipCreatePen1, GdipCreateSolidFill, GdipDeleteBrush, GdipDeleteGraphics, GdipDeletePath,
    GdipDeletePen, GdipDrawPath, GdipFillPath, GdipSetPixelOffsetMode, GdipSetSmoothingMode,
    GdiplusStartup, GdiplusStartupInput, GpGraphics, GpPath, PixelOffsetModeHalf,
    SmoothingModeAntiAlias, UnitPixel,
};

/// Ikony Segoe Fluent Icons (te same kody w Segoe MDL2 Assets).
pub(super) mod glyph {
    /// Tarcza — znak Sędziego Cieni w nagłówku.
    pub const SHIELD: &str = "\u{EA18}";
    /// „?” — opis Sędziego.
    pub const HELP: &str = "\u{E897}";
    /// ✕ — zamknięcie (= odmowa).
    pub const CLOSE: &str = "\u{E711}";
    /// Ryzyko niskie.
    pub const OK: &str = "\u{E930}";
    /// Ryzyko średnie i wysokie.
    pub const WARNING: &str = "\u{E7BA}";
    /// Ryzyko krytyczne.
    pub const STOP: &str = "\u{EA39}";
    /// Informacja (opis pod „?”).
    pub const INFO: &str = "\u{E946}";
}

/// GDI+ uruchamiany raz na proces (okno i podgląd); bez startu kształty rysuje zwykłe GDI.
fn gdiplus() -> bool {
    static STARTED: OnceLock<bool> = OnceLock::new();
    *STARTED.get_or_init(|| {
        let mut token = 0usize;
        let input = GdiplusStartupInput {
            GdiplusVersion: 1,
            ..Default::default()
        };
        // SAFETY: start biblioteki z wartości lokalnych; żeton zostaje do końca procesu.
        unsafe { GdiplusStartup(&raw mut token, &raw const input, std::ptr::null_mut()) }.0 == 0
    })
}

/// COLORREF (0x00BBGGRR) → ARGB GDI+ z kryciem `alpha`.
fn argb(c: COLORREF, alpha: u8) -> u32 {
    let (r, g, b) = (c.0 & 0xFF, (c.0 >> 8) & 0xFF, (c.0 >> 16) & 0xFF);
    (u32::from(alpha) << 24) | (r << 16) | (g << 8) | b
}

/// Mieszanie koloru `a` z `b` (`t` z 0–100: udział `a`).
pub(super) fn mix(a: COLORREF, b: COLORREF, t: u32) -> COLORREF {
    let ch = |c: COLORREF, shift: u32| (c.0 >> shift) & 0xFF;
    let m = |shift: u32| (ch(a, shift) * t + ch(b, shift) * (100 - t)) / 100;
    COLORREF(m(0) | (m(8) << 8) | (m(16) << 16))
}

/// Prostokąt wypełniony jednolicie (bez wygładzania — krawędzie proste).
pub(super) fn fill(hdc: HDC, rect: &RECT, color: COLORREF) {
    // SAFETY: pędzel GDI tego wątku, zwalniany zaraz po wypełnieniu.
    unsafe {
        let b = CreateSolidBrush(color);
        FillRect(hdc, rect, b);
        let _ = DeleteObject(b.into());
    }
}

/// Ścieżka zaokrąglonego prostokąta (promień `radius` w pikselach; promień ≥ połowy wysokości =
/// pigułka albo koło).
fn rounded(path: *mut GpPath, r: &RECT, radius: f32, inset: f32) {
    let (x, y) = (r.left as f32 + inset, r.top as f32 + inset);
    let (w, h) = (
        (r.right - r.left) as f32 - 2.0 * inset,
        (r.bottom - r.top) as f32 - 2.0 * inset,
    );
    let d = (2.0 * radius).min(w).min(h).max(0.5);
    // SAFETY: ścieżka utworzona przez wołającego, żyje do `GdipDeletePath`.
    unsafe {
        let _ = GdipAddPathArc(path, x, y, d, d, 180.0, 90.0);
        let _ = GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0);
        let _ = GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0);
        let _ = GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0);
        let _ = GdipClosePathFigure(path);
    }
}

/// Wygładzony zaokrąglony prostokąt: wypełnienie `body` i obrys `line` grubości `width`
/// (obrys wewnątrz prostokąta). Bez GDI+ — prostokąt GDI.
pub(super) fn round_rect(
    hdc: HDC,
    r: &RECT,
    radius: i32,
    body: Option<COLORREF>,
    line: Option<(COLORREF, i32)>,
) {
    if !gdiplus() {
        if let Some(c) = body.or(line.map(|l| l.0)) {
            fill(hdc, r, c);
        }
        return;
    }
    // SAFETY: obiekty GDI+ tylko tej funkcji, zwalniane przed powrotem; kontekst od wołającego.
    unsafe {
        let mut g: *mut GpGraphics = std::ptr::null_mut();
        if GdipCreateFromHDC(hdc, &raw mut g).0 != 0 || g.is_null() {
            return;
        }
        let _ = GdipSetSmoothingMode(g, SmoothingModeAntiAlias);
        let _ = GdipSetPixelOffsetMode(g, PixelOffsetModeHalf);
        if let Some(c) = body {
            let mut path: *mut GpPath = std::ptr::null_mut();
            let _ = GdipCreatePath(FillModeAlternate, &raw mut path);
            rounded(path, r, radius as f32, 0.0);
            let mut brush = std::ptr::null_mut();
            let _ = GdipCreateSolidFill(argb(c, 255), &raw mut brush);
            let _ = GdipFillPath(g, brush.cast(), path);
            let _ = GdipDeleteBrush(brush.cast());
            let _ = GdipDeletePath(path);
        }
        if let Some((c, w)) = line {
            let half = w as f32 / 2.0;
            let mut path: *mut GpPath = std::ptr::null_mut();
            let _ = GdipCreatePath(FillModeAlternate, &raw mut path);
            rounded(path, r, (radius as f32 - half).max(0.5), half);
            let mut pen = std::ptr::null_mut();
            let _ = GdipCreatePen1(argb(c, 255), w as f32, UnitPixel, &raw mut pen);
            let _ = GdipDrawPath(g, pen, path);
            let _ = GdipDeletePen(pen);
            let _ = GdipDeletePath(path);
        }
        let _ = GdipDeleteGraphics(g);
    }
}

/// Tekst w prostokącie (format jak przy pomiarze).
pub(super) fn text(
    hdc: HDC,
    font: HFONT,
    color: COLORREF,
    s: &str,
    mut r: RECT,
    format: DRAW_TEXT_FORMAT,
) {
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

/// Jeden wiersz wyśrodkowany w prostokącie (ikony, napisy przycisków).
pub(super) const CENTER: DRAW_TEXT_FORMAT =
    DRAW_TEXT_FORMAT(DT_SINGLELINE.0 | DT_VCENTER.0 | DT_CENTER.0);

/// Jeden wiersz wyrównany do lewej, wyśrodkowany w pionie.
pub(super) const LINE: DRAW_TEXT_FORMAT = DRAW_TEXT_FORMAT(DT_SINGLELINE.0 | DT_VCENTER.0);
