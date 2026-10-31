//! Stan okna Sędziego Cieni (bieżąca karta, układ, pomoc pod „?”, przycisk pod kursorem),
//! malowanie w `WM_PAINT` i `WM_DRAWITEM` (oba przez bitmapę w pamięci — bez migotania przy
//! najechaniu) oraz rozmiar i położenie okna na ekranie (okno bez ramki: rozmiar = obszar klienta).

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::Arc;

use platform_contract::SurfaceView;
use windows::Win32::Foundation::{HINSTANCE, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
    EndPaint, GetDC, InvalidateRect, PAINTSTRUCT, ReleaseDC, SRCCOPY, SelectObject,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::EventQueue;
use super::layout::{CLOSE_ID, HELP_ID, Measured, measure};
use super::button::{ButtonState, Kind};
use super::paint::{DrawItem, Look, ODS_FOCUS, ODS_SELECTED};

pub(super) struct Ui {
    pub events: Arc<EventQueue>,
    pub hwnd: Option<HWND>,
    pub controls: Vec<HWND>,
    /// Wygląd (motyw, czcionki, kolor ryzyka) — moduł `paint`.
    pub look: Look,
    pub view: Option<SurfaceView>,
    pub measured: Option<Measured>,
    pub help_open: bool,
    /// Przycisk pod kursorem (podświetlenie).
    pub hover: Option<u16>,
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

/// Rodzaj przycisku o identyfikatorze `id`: główny jest pierwszy przycisk decyzji poza
/// bezpiecznym (fokus startowy — „Odmów”).
pub(super) fn kind(view: &SurfaceView, help_open: bool, id: u16) -> Kind {
    let primary = view.buttons.iter().find(|b| b.id != view.initial_focus).map(|b| b.id);
    match id {
        HELP_ID => Kind::Help(help_open),
        CLOSE_ID => Kind::Close,
        _ if Some(id) == primary => Kind::Primary,
        _ => Kind::Secondary,
    }
}

/// `WM_DRAWITEM`: przycisk decyzji, „?” albo ✕ (przez bitmapę w pamięci).
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
        hover: ui.hover == Some(id),
        kind: kind(view, ui.help_open, id),
    };
    let r = dis.rc_item;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    // SAFETY: bitmapa w pamięci tylko na czas rysowania, zwalniana przed powrotem.
    unsafe {
        let mem = CreateCompatibleDC(Some(dis.hdc));
        let bmp = CreateCompatibleBitmap(dis.hdc, w, h);
        let old = SelectObject(mem, bmp.into());
        let local = RECT { left: 0, top: 0, right: w, bottom: h };
        ui.look.button(mem, local, label, &st);
        let _ = BitBlt(dis.hdc, r.left, r.top, w, h, Some(mem), 0, 0, SRCCOPY);
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
    }
}

/// Odświeża przycisk o identyfikatorze `id` (zmiana podświetlenia).
pub(super) fn redraw(ui: &Ui, id: u16) {
    let Some(hwnd) = ui.hwnd else { return };
    // SAFETY: kontrolka naszego okna (albo brak — wtedy nic).
    unsafe {
        if let Ok(c) = GetDlgItem(Some(hwnd), i32::from(id)) {
            let _ = InvalidateRect(Some(c), None, false);
        }
    }
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
    let m = measure(hdc, &ui.look.fonts, view, ui.help_open, area.bottom - area.top);
    // SAFETY: kontekst pobrany wyżej.
    let _ = unsafe { ReleaseDC(Some(hwnd), hdc) };
    let ids = m.controls();
    for c in &ui.controls {
        // SAFETY: nasza kontrolka.
        let id = u16::try_from(unsafe { GetDlgCtrlID(*c) }).unwrap_or(0);
        if let Some((_, r)) = ids.iter().find(|(i, _)| *i == id) {
            let (x, y, w, h) = Measured::pixel(r);
            // SAFETY: jw.
            let _ = unsafe { MoveWindow(*c, x, y, w, h, true) };
        }
    }
    let (w, h) = m.client;
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
