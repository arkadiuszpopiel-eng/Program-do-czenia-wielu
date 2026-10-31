//! Stan okna Sędziego Cieni (bieżąca karta, układ, pomoc pod „?”), malowanie w `WM_PAINT`
//! i `WM_DRAWITEM` oraz rozmiar i położenie okna na ekranie.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::Arc;

use platform_contract::SurfaceView;
use windows::Win32::Foundation::{HINSTANCE, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
    EndPaint, GetDC, InvalidateRect, PAINTSTRUCT, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::EventQueue;
use super::layout::{HELP_ID, Measured, measure};
use super::paint::{ButtonState, DrawItem, Look, ODS_FOCUS, ODS_SELECTED};

pub(super) struct Ui {
    pub events: Arc<EventQueue>,
    pub hwnd: Option<HWND>,
    pub controls: Vec<HWND>,
    /// Wygląd (motyw, czcionki, kolor ryzyka) — moduł `paint`.
    pub look: Look,
    pub view: Option<SurfaceView>,
    pub measured: Option<Measured>,
    pub help_open: bool,
    pub focus: Option<HWND>,
    pub module: Option<HINSTANCE>,
}

/// `WM_PAINT`: całe okno do bitmapy w pamięci, potem jednym `BitBlt` (bez migotania).
pub(super) fn paint(ui: &Ui, hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    // SAFETY: malowanie naszego okna; obiekty GDI zwalniane przed `EndPaint`.
    unsafe {
        let hdc = BeginPaint(hwnd, &raw mut ps);
        if let (Some(view), Some(m)) = (&ui.view, &ui.measured) {
            let (w, h) = m.client;
            let mem = CreateCompatibleDC(Some(hdc));
            let bmp = CreateCompatibleBitmap(hdc, w, h);
            let old = SelectObject(mem, bmp.into());
            ui.look.paint(mem, view, m);
            let _ = BitBlt(hdc, 0, 0, w, h, Some(mem), 0, 0, SRCCOPY);
            SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
        }
        let _ = EndPaint(hwnd, &raw const ps);
    }
}

/// `WM_DRAWITEM`: przycisk decyzji albo „?”.
pub(super) fn draw_item(ui: &Ui, dis: &DrawItem) {
    let Ok(id) = u16::try_from(dis.ctl_id) else {
        return;
    };
    let Some(view) = &ui.view else { return };
    let label = view
        .buttons
        .iter()
        .find(|b| b.id == id)
        .map_or("", |b| b.label.as_str());
    let st = ButtonState {
        pressed: (dis.item_state & ODS_SELECTED) != 0,
        focused: (dis.item_state & ODS_FOCUS) != 0,
        safe: id == view.initial_focus,
        help: (id == HELP_ID).then_some(ui.help_open),
    };
    ui.look.button(dis.hdc, dis.rc_item, label, &st);
}

pub(super) const FRAME: WINDOW_STYLE =
    WINDOW_STYLE(WS_POPUP.0 | WS_CAPTION.0 | WS_SYSMENU.0 | WS_CLIPCHILDREN.0);

/// Obszar roboczy ekranu głównego (bez paska zadań).
fn work_area() -> RECT {
    let mut r = RECT::default();
    // SAFETY: odczyt prostokąta do zmiennej lokalnej.
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&raw mut r).cast::<c_void>()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if read.is_err() || r.right <= r.left {
        // SAFETY: rozmiar ekranu głównego.
        unsafe {
            r = RECT {
                left: 0,
                top: 0,
                right: GetSystemMetrics(SM_CXSCREEN),
                bottom: GetSystemMetrics(SM_CYSCREEN),
            };
        }
    }
    r
}

/// Mierzy kartę, ustawia przyciski i rozmiar okna. `center` — nowa karta (okno na środku ekranu);
/// inaczej (rozwinięcie pomocy) górna krawędź zostaje, żeby „?” nie uciekał spod kursora.
pub(super) fn arrange(ui: &mut Ui, center: bool) {
    let (Some(hwnd), Some(view)) = (ui.hwnd, ui.view.as_ref()) else {
        return;
    };
    let area = work_area();
    // SAFETY: zapytania o DPI naszego okna i jego kontekst (zwalniany niżej).
    let (dpi, hdc) = unsafe { (GetDpiForWindow(hwnd), GetDC(Some(hwnd))) };
    ui.look.refresh(dpi);
    ui.look.tone = view.tone;
    ui.look.frame(hwnd);
    let mut frame = RECT::default();
    // SAFETY: rozmiar ramki dla DPI do zmiennej lokalnej.
    let _ = unsafe {
        AdjustWindowRectExForDpi(
            &raw mut frame,
            FRAME,
            false,
            WS_EX_TOPMOST | WS_EX_DLGMODALFRAME,
            dpi,
        )
    };
    let chrome_h = frame.bottom - frame.top;
    let m = measure(
        hdc,
        &ui.look.fonts,
        view,
        ui.help_open,
        area.bottom - area.top - chrome_h,
    );
    // SAFETY: kontekst pobrany wyżej.
    let _ = unsafe { ReleaseDC(Some(hwnd), hdc) };
    let mut ids = m.buttons.clone();
    ids.extend(m.help_button.map(|r| (HELP_ID, r)));
    for c in &ui.controls {
        // SAFETY: nasza kontrolka.
        let id = u16::try_from(unsafe { GetDlgCtrlID(*c) }).unwrap_or(0);
        if let Some((_, r)) = ids.iter().find(|(i, _)| *i == id) {
            let (x, y, w, h) = Measured::pixel(r);
            // SAFETY: jw.
            let _ = unsafe { MoveWindow(*c, x, y, w, h, true) };
        }
    }
    let w = m.client.0 + frame.right - frame.left;
    let h = m.client.1 + chrome_h;
    let mut now = RECT::default();
    // SAFETY: położenie naszego okna.
    let _ = unsafe { GetWindowRect(hwnd, &raw mut now) };
    let x = area.left + (area.right - area.left - w) / 2;
    let y = if center {
        area.top + ((area.bottom - area.top - h) / 3).max(0)
    } else {
        now.top.min(area.bottom - h).max(area.top)
    };
    let x = if center { x } else { now.left };
    ui.measured = Some(m);
    // SAFETY: nasze okno.
    unsafe {
        let _ = MoveWindow(hwnd, x, y, w, h, true);
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}
