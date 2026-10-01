//! Wirtualny pulpit (F6): okna z kolejnością Z, monitory, drzewa elementów UIA, kursor,
//! wirtualny zegar, fizyczne wejście użytkownika i skrypty zdarzeń (np. „Broker-UI wyskakuje na
//! wierzch po N paczkach”). Implementuje `DesktopPort`, `UiaPort`, `InputBackend`/`InputPort`
//! i `ScreenCapturePort` z tymi samymi regułami co implementacja Windows (logika z kontraktu).
//!
//! Każde zdarzenie, które **faktycznie** dotarło do okna (wejście, akcja UIA, zmiana okna), jest
//! zapisywane w [`GuiRecord`] z PID-em i obrazem procesu okna — testy właściwościowe liczą
//! akcje wobec okien chronionych na podstawie skutków, nie deklaracji.

mod capture;
mod input;
mod uia;
mod windows;

use std::sync::{Mutex, MutexGuard};

use platform_contract::{
    DesktopWindow, ElementRef, GuiError, InputPacing, MonitorInfo, RgbaImage, ScreenRect,
    TargetGuard, UiaNode, WindowId, WindowState,
};

pub use capture::{DESKTOP_COLOR, PASSWORD_COLOR};
pub use uia::FakeElement;

/// Bazowy PID okien atrapy (poza zakresem PID-ów Linuksa i Windows w testach).
pub const FAKE_PID_BASE: u32 = 3_000_000_000;

/// Okno do dodania.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeWindow {
    /// Tytuł.
    pub title: String,
    /// Obraz procesu.
    pub image: String,
    /// Prostokąt.
    pub rect: ScreenRect,
    /// Proces podniesiony.
    pub elevated: bool,
    /// Kolor wypełnienia na zrzucie.
    pub color: [u8; 4],
    /// Okno chronione przed przechwyceniem (czarna klatka).
    pub capture_blocked: bool,
}

impl FakeWindow {
    /// Okno o tytule, obrazie i prostokącie (kolor szary).
    pub fn new(title: &str, image: &str, rect: ScreenRect) -> Self {
        Self {
            title: title.into(),
            image: image.into(),
            rect,
            elevated: false,
            color: [200, 200, 200, 255],
            capture_blocked: false,
        }
    }
}

/// Rodzaj skutku zapisanego w dzienniku.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuiRecordKind {
    /// Zdarzenie wejścia (`RawInput` jako tekst Debug).
    Input(String),
    /// Akcja UIA (nazwa).
    Uia(String),
    /// Zmiana okna (`focus`, `bounds`, `state`).
    Window(String),
}

/// Skutek, który dotarł do okna.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuiRecord {
    /// Okno.
    pub window: WindowId,
    /// PID procesu okna.
    pub pid: u32,
    /// Obraz procesu okna.
    pub image: String,
    /// Rodzaj.
    pub kind: GuiRecordKind,
}

/// Zdarzenie skryptu po N wstrzykniętych paczkach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptEvent {
    /// Użytkownik dotyka myszy/klawiatury (wejście fizyczne).
    PhysicalInput,
    /// Okno wyskakuje na wierzch i przejmuje fokus (np. Broker-UI).
    Raise(WindowId),
}

#[derive(Debug)]
struct Win {
    info: DesktopWindow,
    color: [u8; 4],
    capture_blocked: bool,
    restore: ScreenRect,
    elements: Vec<FakeElement>,
    typed: Vec<u16>,
}

#[derive(Debug)]
struct State {
    windows: Vec<Win>,
    monitors: Vec<MonitorInfo>,
    clock: u64,
    physical: Option<u64>,
    cursor: (i32, i32),
    batches: u64,
    script: Vec<(u64, ScriptEvent)>,
    records: Vec<GuiRecord>,
    uia_hang: bool,
    password_check_fails: Vec<WindowId>,
    last_capture: Option<RgbaImage>,
    next_id: u64,
}

impl State {
    fn win(&self, id: WindowId) -> Result<&Win, GuiError> {
        self.windows
            .iter()
            .find(|w| w.info.id == id)
            .ok_or_else(|| GuiError::ElementNotFound(format!("okno {}", id.0)))
    }

    fn win_mut(&mut self, id: WindowId) -> Result<&mut Win, GuiError> {
        self.windows
            .iter_mut()
            .find(|w| w.info.id == id)
            .ok_or_else(|| GuiError::ElementNotFound(format!("okno {}", id.0)))
    }

    fn raise(&mut self, id: WindowId) {
        if let Some(pos) = self.windows.iter().position(|w| w.info.id == id) {
            let mut w = self.windows.remove(pos);
            if w.info.state == WindowState::Minimized {
                w.info.state = WindowState::Normal;
            }
            self.windows.insert(0, w);
        }
        self.renumber(Some(id));
    }

    fn renumber(&mut self, focus: Option<WindowId>) {
        let focus = focus.or_else(|| {
            self.windows
                .iter()
                .find(|w| w.info.state != WindowState::Minimized)
                .map(|w| w.info.id)
        });
        for (i, w) in self.windows.iter_mut().enumerate() {
            w.info.z_order = u32::try_from(i).unwrap_or(u32::MAX);
            w.info.focused = Some(w.info.id) == focus && w.info.state != WindowState::Minimized;
        }
    }

    fn top_at(&self, x: i32, y: i32) -> Option<&Win> {
        self.windows
            .iter()
            .find(|w| w.info.state != WindowState::Minimized && w.info.rect.contains(x, y))
    }

    fn record(&mut self, id: WindowId, kind: GuiRecordKind) {
        if let Ok(w) = self.win(id) {
            let r = GuiRecord {
                window: id,
                pid: w.info.pid,
                image: w.info.image.clone(),
                kind,
            };
            self.records.push(r);
        }
    }
}

/// Wirtualny pulpit.
#[derive(Debug)]
pub struct FakeDesktop {
    state: Mutex<State>,
    guard: TargetGuard,
    pacing: InputPacing,
}

impl Default for FakeDesktop {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeDesktop {
    /// Pulpit z jednym monitorem 1920×1080 i strażnikiem bazowym.
    pub fn new() -> Self {
        Self::with_guard(TargetGuard::baseline())
    }

    /// Pulpit z własnym strażnikiem.
    pub fn with_guard(guard: TargetGuard) -> Self {
        let monitor = MonitorInfo {
            index: 0,
            rect: ScreenRect::from_xywh(0, 0, 1920, 1080),
            work_area: ScreenRect::from_xywh(0, 0, 1920, 1040),
            dpi: 96,
            primary: true,
        };
        Self {
            state: Mutex::new(State {
                windows: Vec::new(),
                monitors: vec![monitor],
                clock: 1_000_000,
                physical: None,
                cursor: (0, 0),
                batches: 0,
                script: Vec::new(),
                records: Vec::new(),
                uia_hang: false,
                password_check_fails: Vec::new(),
                last_capture: None,
                next_id: 1,
            }),
            guard,
            pacing: InputPacing::default(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Dodaje okno na wierzch (z fokusem, jeśli `focus`).
    pub fn add_window(&self, spec: FakeWindow, focus: bool) -> WindowId {
        let mut s = self.lock();
        let id = WindowId(s.next_id);
        s.next_id += 1;
        let pid = FAKE_PID_BASE + u32::try_from(id.0).unwrap_or(0);
        let info = DesktopWindow {
            id,
            title: spec.title,
            class_name: "FakeWindow".into(),
            pid,
            protected: self.guard.is_protected(pid, &spec.image),
            image: spec.image,
            rect: spec.rect,
            monitor: 0,
            dpi: 96,
            state: WindowState::Normal,
            focused: false,
            z_order: 0,
            elevated: spec.elevated,
        };
        let current = s.windows.iter().find(|w| w.info.focused).map(|w| w.info.id);
        s.windows.insert(
            0,
            Win {
                info,
                color: spec.color,
                capture_blocked: spec.capture_blocked,
                restore: spec.rect,
                elements: Vec::new(),
                typed: Vec::new(),
            },
        );
        s.renumber(if focus { Some(id) } else { current });
        id
    }

    /// Użytkownik dotyka myszy/klawiatury teraz (wejście fizyczne, niewstrzyknięte).
    pub fn physical_input(&self) {
        let mut s = self.lock();
        s.physical = Some(s.clock);
    }

    /// Przesuwa wirtualny zegar.
    pub fn advance(&self, ms: u64) {
        self.lock().clock += ms;
    }

    /// Zdarzenie po `batches` wstrzykniętych paczkach (licząc od początku życia atrapy).
    pub fn script_after(&self, batches: u64, event: ScriptEvent) {
        self.lock().script.push((batches, event));
    }

    /// Liczba wstrzykniętych paczek.
    pub fn injected_batches(&self) -> u64 {
        self.lock().batches
    }

    /// UIA „wisi” (każde wywołanie kończy się limitem czasu).
    pub fn set_uia_hang(&self, hang: bool) {
        self.lock().uia_hang = hang;
    }

    /// Sprawdzenie pól haseł okna się nie udaje (zrzut maskuje całe okno).
    pub fn fail_password_check(&self, window: WindowId) {
        self.lock().password_check_fails.push(window);
    }

    /// Skutki, które dotarły do okien.
    pub fn records(&self) -> Vec<GuiRecord> {
        self.lock().records.clone()
    }

    /// Tekst wpisany do okna (z wejścia syntetycznego).
    pub fn typed_text(&self, window: WindowId) -> String {
        self.lock()
            .win(window)
            .map(|w| String::from_utf16_lossy(&w.typed))
            .unwrap_or_default()
    }

    /// Ostatni zrzut po maskowaniu i skalowaniu (przed PNG) — do asercji pikseli.
    pub fn last_capture(&self) -> Option<RgbaImage> {
        self.lock().last_capture.clone()
    }

    /// Dodaje element do drzewa okna; zwraca odwołanie.
    pub fn add_element(&self, window: WindowId, element: FakeElement) -> Option<ElementRef> {
        let mut s = self.lock();
        let w = s.win_mut(window).ok()?;
        let index = i32::try_from(w.elements.len()).unwrap_or(i32::MAX);
        let id = i32::try_from(window.0).unwrap_or(i32::MAX);
        let element = element.attach(
            ElementRef {
                window,
                runtime_id: vec![42, id, index],
            },
            w.info.pid,
        );
        let r = element.node.element.clone();
        w.elements.push(element);
        Some(r)
    }

    /// Bieżący stan elementu (bez redakcji — tylko testy).
    pub fn raw_element(&self, element: &ElementRef) -> Option<UiaNode> {
        let s = self.lock();
        let w = s.win(element.window).ok()?;
        w.elements
            .iter()
            .find(|e| e.node.element == *element)
            .map(|e| e.node.clone())
    }
}
