//! Okno Broker-UI (Win32) na własnym wątku z pętlą komunikatów: kontrolki STATIC z
//! `SS_NOPREFIX` (`&` z tekstu LLM nie staje się skrótem) i BUTTON bez przycisku domyślnego
//! (`Enter` → `IDOK`, ignorowany), `IsDialogMessageW` (Tab/Spacja/Esc), topmost, bez aktywacji
//! poza `take_focus` (wtedy próba przejęcia fokusu; zawsze miganie). Układ liczy kontrakt
//! (`SurfaceView::layout`), próbki wejścia — moduł `input`.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender};

use platform_contract::{MIN_BUTTON_ID, PixelRect, SurfaceEvent, SurfaceView};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, GetSysColorBrush, HDC, HFONT, InvalidateRect};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use super::input::{
    begin_dispatch, decision_sample, end_dispatch, install_hooks, occluded, remove_hooks,
};
use super::paint::{DrawItem, Look};
use super::thread::{Request, WM_REQUEST};
use super::{EventQueue, now_ms};
use crate::win::{pcwstr, wide};

const CLASS: PCWSTR = w!("AlfaBrokerUi");
/// `SS_LEFT | SS_NOPREFIX` (winuser.h; bez feature `Win32_UI_Controls`).
const STATIC_PLAIN: u32 = 0x0080;

struct Ui {
    events: Arc<EventQueue>,
    hwnd: Option<HWND>,
    controls: Vec<HWND>,
    /// Wygląd (motyw, kolor ryzyka, przyciski rysowane samodzielnie) — moduł `paint`.
    look: Look,
    focus: Option<HWND>,
    module: Option<HINSTANCE>,
}

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
        WM_COMMAND if low >= MIN_BUTTON_ID && clicked => {
            let (input, occluded) = (decision_sample(), occluded(hwnd));
            push(SurfaceEvent::Button {
                id: low,
                input,
                occluded,
            });
        }
        WM_COMMAND => {}
        // Krzyżyk (WM_NCLBUTTONUP → WM_SYSCOMMAND → WM_CLOSE przez SendMessage) i Alt+F4: próbka
        // z komunikatu wejścia jak przy „Odmów” — w samym WM_CLOSE Windows nie podaje źródła.
        WM_CLOSE => push(SurfaceEvent::Cancel {
            input: decision_sample(),
        }),
        WM_DESTROY => {
            push(SurfaceEvent::Closed);
            with_ui(|ui| ui.hwnd = None);
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            let hdc = HDC(wparam.0 as *mut c_void);
            if let Some(brush) = with_ui(|ui| ui.look.control_colors(hdc, lparam.0)) {
                return LRESULT(brush.0 as isize);
            }
            // SAFETY: domyślna obsługa (stan okna zajęty w trakcie układania).
            return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        WM_ERASEBKGND => {
            let hdc = HDC(wparam.0 as *mut c_void);
            let mut client = RECT::default();
            // SAFETY: obszar klienta naszego okna do zmiennej lokalnej.
            let _ = unsafe { GetClientRect(hwnd, &raw mut client) };
            with_ui(|ui| ui.look.erase(hdc, client));
            return LRESULT(1);
        }
        WM_DRAWITEM if lparam.0 != 0 => {
            // SAFETY: dla `WM_DRAWITEM` `lparam` wskazuje na `DRAWITEMSTRUCT` na czas komunikatu.
            let dis = unsafe { &*(lparam.0 as *const DrawItem) };
            with_ui(|ui| ui.look.draw(dis));
            return LRESULT(1);
        }
        // SAFETY: domyślna obsługa pozostałych komunikatów.
        _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
    LRESULT(0)
}

/// Okno główne (`parent = None`) albo kontrolka: (klasa, tekst, styl, identyfikator).
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

/// Układa okno od nowa (kontrolki tworzone ponownie, okno główne zostaje).
fn layout(ui: &mut Ui, view: &SurfaceView) -> Option<HWND> {
    let frame = WS_POPUP | WS_CAPTION | WS_SYSMENU;
    let hwnd = match ui.hwnd {
        Some(h) => h,
        None => create(ui, None, (CLASS, &view.title, frame, 0), (0, 0, 100, 100))?,
    };
    ui.hwnd = Some(hwnd);
    for c in ui.controls.drain(..) {
        // SAFETY: nasze kontrolki potomne.
        let _ = unsafe { DestroyWindow(c) };
    }
    // SAFETY: zapytania o DPI okna i rozmiar ekranu.
    let (dpi, sw, sh) = unsafe {
        (
            GetDpiForWindow(hwnd),
            GetSystemMetrics(SM_CXSCREEN),
            GetSystemMetrics(SM_CYSCREEN),
        )
    };
    let l = view.layout(dpi);
    let text = WS_CHILD | WS_VISIBLE | WINDOW_STYLE(STATIC_PLAIN);
    // Przyciski rysowane samodzielnie (`paint`); kliknięcie, Spacja i fokus działają jak w zwykłych.
    let push_style = WINDOW_STYLE(u32::try_from(BS_OWNERDRAW).unwrap_or(0));
    let button = WS_CHILD | WS_VISIBLE | WS_TABSTOP | push_style;
    let fonts = ui.look.prepare(hwnd, dpi);
    let (f_title, f_badge, f_body) = (fonts.title, fonts.badge, fonts.body);
    let (f_hint, f_button) = (fonts.hint, fonts.button);
    let stat =
        |ui: &Ui, t: &str, r: PixelRect| create(ui, Some(hwnd), (w!("STATIC"), t, text, 0), r);
    let mut made: Vec<(Option<HWND>, HFONT)> = Vec::new();
    let title_c = stat(ui, &view.title, l.title);
    made.push((title_c, f_title));
    ui.look.badge = stat(ui, &view.badge, l.badge);
    made.push((ui.look.badge, f_badge));
    // Siatka „etykieta / wartość” jak w karcie zgody `ui-kit` (etykiety przygaszone).
    for ((k, v), (lr, vr)) in view.details.iter().zip(&l.rows) {
        let label = stat(ui, k, *lr);
        ui.look.muted.extend(label);
        made.push((label, f_body));
        made.push((stat(ui, v, *vr), f_body));
    }
    ui.look.status = stat(ui, &view.status, l.status);
    made.push((ui.look.status, f_hint));
    ui.focus = None;
    for (b, (id, r)) in view.buttons.iter().zip(l.buttons) {
        ui.look.labels.push((id, b.label.encode_utf16().collect()));
        let c = create(ui, Some(hwnd), (w!("BUTTON"), &b.label, button, id), r);
        if id == view.initial_focus {
            ui.focus = c;
        }
        made.push((c, f_button));
    }
    ui.look.card = l.card;
    ui.look.tone = view.tone;
    ui.look.safe = view.initial_focus;
    for (c, font) in made {
        let Some(c) = c else { continue };
        // SAFETY: nasza kontrolka; czcionka żyje do następnego ułożenia (`Look::prepare`).
        unsafe {
            SendMessageW(
                c,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            )
        };
        ui.controls.push(c);
    }
    let title = wide(&view.title);
    let (w, h) = l.size;
    // SAFETY: nasze okno; tytuł żyje przez wywołanie.
    unsafe {
        let _ = SetWindowTextW(hwnd, pcwstr(&title));
        let _ = MoveWindow(hwnd, (sw - w) / 2, (sh - h) / 3, w, h, true);
        let _ = InvalidateRect(Some(hwnd), None, true);
    }
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
        // SAFETY: kursor i pędzel systemowe (nie zwalnia się ich).
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
        // SAFETY: jw.
        hbrBackground: unsafe { GetSysColorBrush(COLOR_WINDOW) },
        lpszClassName: CLASS,
        ..Default::default()
    };
    // SAFETY: rejestracja klasy (ponowna w tym samym procesie zwraca 0 — klasa już jest).
    let _ = unsafe { RegisterClassExW(&raw const class) };
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
        look: Look::new(),
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
        // SAFETY: nasze okno; niszczone przed zwolnieniem czcionek i pędzli (`Look::drop`).
        let _ = unsafe { DestroyWindow(h) };
    }
    drop(ui);
}
