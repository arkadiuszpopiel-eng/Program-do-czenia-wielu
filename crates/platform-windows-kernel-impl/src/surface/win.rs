//! Okno Sędziego Cieni (Win32) na własnym wątku z pętlą komunikatów: bez systemowej belki
//! (`WM_NCCALCSIZE` → całe okno to obszar klienta; cień i zaokrąglone rogi zostają z DWM,
//! przeciąganie za nagłówek przez `HTCAPTION`), cały tekst rysowany w `WM_PAINT` (moduł `paint`,
//! układ z `layout` mierzony prawdziwą czcionką), jedyne kontrolki to przyciski BUTTON rysowane
//! samodzielnie bez przycisku domyślnego (`Enter` → `IDOK`, ignorowany), „?” (`IDHELP`, też F1)
//! rozwijający opis Sędziego i ✕ (`IDCANCEL` — jak Esc, odmowa). Podświetlenie przycisku pod
//! kursorem: `WM_MOUSEMOVE` z pętli komunikatów i zegar sprawdzający zejście kursora. `IsDialogMessageW` (Tab/Spacja/Esc),
//! topmost, bez aktywacji poza `take_focus` (wtedy próba przejęcia fokusu; zawsze miganie).
//! Próbki wejścia — moduł `input`.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender};

use platform_contract::{MIN_BUTTON_ID, PixelRect, SurfaceEvent, SurfaceView};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_F1};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use super::input::{
    begin_dispatch, decision_sample, end_dispatch, install_hooks, occluded, remove_hooks,
};
use super::layout::{CLOSE_ID, HELP_ID};
use super::paint::{DrawItem, Look, WINDOW_NAME};
use super::pointer::{caption_hit, control_of, hover_now};
use super::state::{FRAME, Ui, arrange, draw_item, paint, redraw};
use super::theme::palette;
use super::thread::{Request, WM_REQUEST};
use super::{EventQueue, now_ms};
use crate::win::{pcwstr, wide};

const CLASS: PCWSTR = w!("AlfaBrokerUi");
/// Nazwa dostępna przycisku „?” (czytnik ekranu, podpowiedź).
const HELP_LABEL: &str = "Co to jest Sędzia Cieni? (F1)";
/// Nazwa dostępna ✕ w nagłówku.
const CLOSE_LABEL: &str = "Odmów i zamknij (Esc)";
/// Zegar sprawdzający, czy kursor zszedł z podświetlonego przycisku.
const HOVER_TIMER: usize = 7;

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
}

fn with_ui<R>(f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    UI.with(|c| c.try_borrow_mut().ok()?.as_mut().map(f))
}

fn push(event: SurfaceEvent) {
    with_ui(|ui| ui.events.push(event));
}

fn focus_initial() {
    if let Some(Some(f)) = with_ui(|ui| ui.focus) {
        // SAFETY: fokus na kontrolce naszego okna (ten sam wątek).
        let _ = unsafe { SetFocus(Some(f)) };
    }
}

fn toggle_help() {
    with_ui(|ui| {
        ui.help_open = !ui.help_open;
        arrange(ui, false);
    });
}

/// Podświetla przycisk pod kursorem (albo żaden); zegar pilnuje zejścia kursora z przycisku.
fn set_hover(dialog: HWND, id: Option<u16>) {
    with_ui(|ui| {
        if ui.hover != id {
            let old = std::mem::replace(&mut ui.hover, id);
            for i in [old, id].into_iter().flatten() {
                redraw(ui, i);
            }
        }
    });
    // SAFETY: zegar naszego okna.
    unsafe {
        if id.is_some() {
            let _ = SetTimer(Some(dialog), HOVER_TIMER, 60, None);
        } else {
            let _ = KillTimer(Some(dialog), HOVER_TIMER);
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let low = u16::try_from(wparam.0 & 0xFFFF).unwrap_or(0);
    // Przycisk rysowany samodzielnie zgłasza szybkie drugie kliknięcie jako `BN_DOUBLECLICKED` —
    // to też naciśnięcie (ta sama próbka wejścia i te same reguły), inaczej by „ginęło”.
    let code = (wparam.0 >> 16) & 0xFFFF;
    let clicked = code == BN_CLICKED as usize || code == BN_DOUBLECLICKED as usize;
    match msg {
        WM_ACTIVATE if u32::from(low) == WA_INACTIVE => {
            push(SurfaceEvent::Deactivated { at_ms: now_ms() });
        }
        WM_ACTIVATE => {
            push(SurfaceEvent::Activated { at_ms: now_ms() });
            focus_initial();
        }
        WM_COMMAND if i32::from(low) == IDCANCEL.0 => {
            push(SurfaceEvent::Cancel {
                input: decision_sample(),
            });
        }
        // „?” nigdy nie jest decyzją: tylko rozwija albo zwija opis.
        WM_COMMAND if low == HELP_ID && clicked => toggle_help(),
        WM_COMMAND if low >= MIN_BUTTON_ID && clicked => {
            let (input, occluded) = (decision_sample(), occluded(hwnd));
            push(SurfaceEvent::Button {
                id: low,
                input,
                occluded,
            });
        }
        WM_COMMAND => {}
        WM_HELP => toggle_help(),
        // Krzyżyk (WM_NCLBUTTONUP → WM_SYSCOMMAND → WM_CLOSE przez SendMessage) i Alt+F4: próbka
        // z komunikatu wejścia jak przy „Odmów” — w samym WM_CLOSE Windows nie podaje źródła.
        WM_CLOSE => push(SurfaceEvent::Cancel {
            input: decision_sample(),
        }),
        WM_DESTROY => {
            push(SurfaceEvent::Closed);
            with_ui(|ui| ui.hwnd = None);
        }
        WM_ERASEBKGND => return LRESULT(1),
        // Bez ramki systemowej: całe okno to obszar klienta (cień i rogi rysuje DWM).
        WM_NCCALCSIZE => {}
        // SAFETY: domyślna obsługa bez rysowania ramki (`lParam = -1`), stan aktywności zostaje.
        WM_NCACTIVATE => return unsafe { DefWindowProcW(hwnd, msg, wparam, LPARAM(-1)) },
        WM_NCHITTEST => {
            // SAFETY: domyślna obsługa; w nagłówku zamiast obszaru klienta — belka tytułu.
            let hit = unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            if hit.0 == HTCLIENT as isize {
                let header = with_ui(|ui| ui.measured.as_ref().map(|m| m.header.bottom)).flatten();
                return header
                    .and_then(|h| caption_hit(hwnd, lparam, h))
                    .unwrap_or(hit);
            }
            return hit;
        }
        WM_TIMER if wparam.0 == HOVER_TIMER => set_hover(hwnd, hover_now(hwnd)),
        WM_PAINT => {
            if with_ui(|ui| paint(ui, hwnd)).is_none() {
                // SAFETY: domyślna obsługa (stan okna zajęty w trakcie układania).
                return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            }
        }
        WM_DRAWITEM if lparam.0 != 0 => {
            // SAFETY: dla `WM_DRAWITEM` `lparam` wskazuje na `DRAWITEMSTRUCT` na czas komunikatu.
            let dis = unsafe { &*(lparam.0 as *const DrawItem) };
            with_ui(|ui| draw_item(ui, dis));
            return LRESULT(1);
        }
        // SAFETY: domyślna obsługa pozostałych komunikatów.
        _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    LRESULT(0)
}

/// Okno główne (`parent = None`) albo przycisk: (klasa, tekst, styl, identyfikator).
fn create(
    ui: &Ui,
    parent: Option<HWND>,
    spec: (PCWSTR, &str, WINDOW_STYLE, u16),
    r: PixelRect,
) -> Option<HWND> {
    let (class, text, style, id) = spec;
    let text = wide(text);
    let menu = parent.map(|_| HMENU(usize::from(id) as *mut c_void));
    let ex = match parent {
        Some(_) => WINDOW_EX_STYLE(0),
        None => WS_EX_TOPMOST | WS_EX_DLGMODALFRAME,
    };
    let (x, y, w, h) = r;
    // SAFETY: tekst zakończony zerem żyje przez wywołanie; klasa zarejestrowana na tym wątku.
    let made = unsafe {
        CreateWindowExW(
            ex,
            class,
            pcwstr(&text),
            style,
            x,
            y,
            w,
            h,
            parent,
            menu,
            ui.module,
            None,
        )
    };
    made.ok()
}

/// Nowa karta: przyciski tworzone od nowa (okno główne zostaje), pomoc zwinięta.
fn layout(ui: &mut Ui, view: &SurfaceView) -> Option<HWND> {
    let hwnd = match ui.hwnd {
        Some(h) => h,
        None => {
            let h = create(ui, None, (CLASS, WINDOW_NAME, FRAME, 0), (0, 0, 100, 100))?;
            let flags = SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;
            // SAFETY: nasze okno — ponowne przeliczenie ramki (bez belki systemowej).
            let _ = unsafe { SetWindowPos(h, None, 0, 0, 0, 0, flags) };
            h
        }
    };
    ui.hwnd = Some(hwnd);
    for c in ui.controls.drain(..) {
        // SAFETY: nasze kontrolki potomne.
        let _ = unsafe { DestroyWindow(c) };
    }
    // Przyciski rysowane samodzielnie (`paint`); kliknięcie, Spacja i fokus działają jak w zwykłych.
    let owner = WINDOW_STYLE(u32::try_from(BS_OWNERDRAW).unwrap_or(0));
    let button = WS_CHILD | WS_VISIBLE | WS_TABSTOP | owner;
    let none = (0, 0, 1, 1);
    ui.focus = None;
    // Kolejność Tab: decyzje, potem „?” i ✕.
    for b in &view.buttons {
        let c = create(ui, Some(hwnd), (w!("BUTTON"), &b.label, button, b.id), none);
        if b.id == view.initial_focus {
            ui.focus = c;
        }
        ui.controls.extend(c);
    }
    if !view.help.is_empty() {
        let c = create(
            ui,
            Some(hwnd),
            (w!("BUTTON"), HELP_LABEL, button, HELP_ID),
            none,
        );
        ui.controls.extend(c);
    }
    let close = (w!("BUTTON"), CLOSE_LABEL, button, CLOSE_ID);
    let c = create(ui, Some(hwnd), close, none);
    ui.controls.extend(c);
    ui.view = Some(view.clone());
    ui.help_open = false;
    ui.hover = None;
    let caption = wide(format!("{WINDOW_NAME} — {}", view.title));
    // SAFETY: nasze okno; tytuł żyje przez wywołanie.
    let _ = unsafe { SetWindowTextW(hwnd, pcwstr(&caption)) };
    arrange(ui, true);
    Some(hwnd)
}

fn present(view: &SurfaceView) {
    let Some(Some(hwnd)) = with_ui(|ui| layout(ui, view)) else {
        push(SurfaceEvent::Closed);
        return;
    };
    let flash = FLASHWINFO {
        cbSize: u32::try_from(size_of::<FLASHWINFO>()).unwrap_or(0),
        hwnd,
        dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
        uCount: 3,
        dwTimeout: 0,
    };
    // SAFETY: nasze okno; bez aktywacji albo (wysokie ryzyko) z próbą przejęcia fokusu.
    let active = unsafe {
        if view.take_focus {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
        } else {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        let _ = FlashWindowEx(&raw const flash);
        GetForegroundWindow() == hwnd
    };
    if active {
        focus_initial();
    }
}

fn on_request(request: Request) {
    let hwnd = with_ui(|ui| ui.hwnd).flatten();
    match (request, hwnd) {
        (Request::Present(view), _) => present(&view),
        (Request::Dismiss, Some(h)) => {
            // SAFETY: nasze okno.
            let _ = unsafe { ShowWindow(h, SW_HIDE) };
        }
        (Request::Dismiss, None) => {}
    }
}

fn register_class(module: Option<HINSTANCE>) {
    let class = WNDCLASSEXW {
        cbSize: u32::try_from(size_of::<WNDCLASSEXW>()).unwrap_or(0),
        lpfnWndProc: Some(wndproc),
        hInstance: module.unwrap_or_default(),
        // SAFETY: kursor systemowy (nie zwalnia się go).
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
        lpszClassName: CLASS,
        ..Default::default()
    };
    // SAFETY: rejestracja klasy (ponowna w tym samym procesie zwraca 0 — klasa już jest).
    let _ = unsafe { RegisterClassExW(&raw const class) };
}

/// F1 w oknie (fokus na przycisku) rozwija opis — zamiast liczyć na `WM_HELP` od systemu.
fn is_help_key(msg: &MSG, dialog: Option<HWND>) -> bool {
    let ours = |h: HWND| {
        // SAFETY: zapytanie o rodzica okna adresata.
        dialog.is_some_and(|d| h == d || unsafe { GetParent(h) }.is_ok_and(|p| p == d))
    };
    msg.message == WM_KEYDOWN && msg.wParam.0 == usize::from(VK_F1.0) && ours(msg.hwnd)
}

pub(super) fn run(inbox: &Receiver<Request>, events: Arc<EventQueue>, ready: &SyncSender<u32>) {
    let mut msg = MSG::default();
    // SAFETY: utworzenie kolejki komunikatów wątku przed zgłoszeniem gotowości.
    let _ = unsafe { PeekMessageW(&raw mut msg, None, WM_USER, WM_USER, PM_NOREMOVE) };
    // SAFETY: uchwyt modułu bieżącego procesu.
    let module: Option<HINSTANCE> = unsafe { GetModuleHandleW(None) }.ok().map(Into::into);
    register_class(module);
    let hooks = install_hooks(module);
    let ui = Ui {
        events,
        hwnd: None,
        controls: Vec::new(),
        look: Look::new(96, palette()),
        view: None,
        measured: None,
        help_open: false,
        hover: None,
        focus: None,
        module,
    };
    UI.with(|c| *c.borrow_mut() = Some(ui));
    // SAFETY: identyfikator bieżącego wątku.
    let _ = ready.send(unsafe { GetCurrentThreadId() });
    // SAFETY: pętla komunikatów bieżącego wątku; `IsDialogMessageW` obsługuje Tab/Spacja/Esc.
    while unsafe { GetMessageW(&raw mut msg, None, 0, 0) }.0 > 0 {
        if msg.message == WM_REQUEST {
            while let Ok(r) = inbox.try_recv() {
                on_request(r);
            }
            continue;
        }
        let dialog = with_ui(|ui| ui.hwnd).flatten();
        if let (WM_MOUSEMOVE, Some(d)) = (msg.message, dialog) {
            set_hover(d, control_of(d, msg.hwnd));
        }
        if is_help_key(&msg, dialog) {
            toggle_help();
            continue;
        }
        begin_dispatch(msg.message);
        // SAFETY: jw.
        unsafe {
            if !dialog.is_some_and(|h| IsDialogMessageW(h, &raw const msg).as_bool()) {
                let _ = TranslateMessage(&raw const msg);
                DispatchMessageW(&raw const msg);
            }
        }
        end_dispatch();
    }
    let ui = UI.with(|c| c.borrow_mut().take());
    remove_hooks(hooks);
    if let Some(h) = ui.as_ref().and_then(|s| s.hwnd) {
        // SAFETY: nasze okno; niszczone przed zwolnieniem czcionek (`Fonts::drop`).
        let _ = unsafe { DestroyWindow(h) };
    }
    drop(ui);
}
