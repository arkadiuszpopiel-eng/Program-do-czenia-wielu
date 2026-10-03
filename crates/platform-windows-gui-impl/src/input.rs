//! Wejście syntetyczne Windows: `InputBackend` dla wspólnego `execute_input` z kontraktu —
//! paczka = jedno `SendInput` (atomowo, bez przeplatania z innym wejściem), mysz w
//! współrzędnych bezwzględnych pulpitu wirtualnego (proces musi być per-monitor DPI aware —
//! powłoka Tauri jest), cel paczki = okno z fokusem / okno pod punktem tuż przed wysłaniem.
//! UIPI po cichu odrzuca wejście do okien o wyższej integralności (Broker-UI) — dodatkowa
//! ochrona poza strażnikiem. Cel paczki ma procesy powiązane (WebView2 Alfy, właściciel, treść
//! UWP — P2-01), a przed paczką wpisującą treść element z fokusem jest czytany przez UIA
//! (`IsPassword`; błąd albo limit czasu = fokus nieznany → odmowa, P2-03).

#![allow(unsafe_code)]

use std::mem::size_of;

use platform_contract::{
    FocusedField, GuiError, InputBackend, MouseButton, PlatformError, RawInput, TargetWindow,
    is_extended_vk,
};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MOUSE_EVENT_FLAGS,
    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WindowFromPoint,
};

use crate::hook::ActivityMonitor;
use crate::links::target_of;
use crate::uia::{self, UiaHost};
use crate::win::{last_error, now_ms};

/// Znacznik w `dwExtraInfo` wejścia Alfy (diagnostyka).
const ALFA_INPUT_MARK: usize = 0xA1FA_0001;

/// Backend `SendInput` + hook aktywności.
pub(crate) struct WinInputBackend<'a> {
    pub(crate) activity: &'a ActivityMonitor,
    /// Wątek UIA (element z fokusem przed paczką wpisującą treść).
    pub(crate) uia: &'a UiaHost,
    /// Limit odczytu fokusu (ms).
    pub(crate) uia_ms: u64,
}

fn keyboard(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: ALFA_INPUT_MARK,
            },
        },
    }
}

fn mouse(dx: i32, dy: i32, data: i32, flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: u32::from_ne_bytes(data.to_ne_bytes()),
                dwFlags: flags,
                time: 0,
                dwExtraInfo: ALFA_INPUT_MARK,
            },
        },
    }
}

/// Punkt ekranu → współrzędne znormalizowane 0–65535 pulpitu wirtualnego.
fn normalize(x: i32, y: i32) -> (i32, i32) {
    // SAFETY: odczyt metryk systemu.
    let (vx, vy, vw, vh) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN).max(2),
            GetSystemMetrics(SM_CYVIRTUALSCREEN).max(2),
        )
    };
    let scale = |v: i32, origin: i32, size: i32| {
        let n = (i64::from(v) - i64::from(origin)) * 65_535 / i64::from(size - 1);
        i32::try_from(n.clamp(0, 65_535)).unwrap_or(0)
    };
    (scale(x, vx, vw), scale(y, vy, vh))
}

fn convert(event: &RawInput) -> INPUT {
    match *event {
        RawInput::Key { vk, up } => {
            let mut flags = KEYBD_EVENT_FLAGS(0);
            if up {
                flags |= KEYEVENTF_KEYUP;
            }
            if is_extended_vk(vk) {
                flags |= KEYEVENTF_EXTENDEDKEY;
            }
            keyboard(vk, 0, flags)
        }
        RawInput::Unicode { unit, up } => {
            let flags = if up {
                KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
            } else {
                KEYEVENTF_UNICODE
            };
            keyboard(0, unit, flags)
        }
        RawInput::MoveTo { x, y } => {
            let (dx, dy) = normalize(x, y);
            mouse(
                dx,
                dy,
                0,
                MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
            )
        }
        RawInput::Button { button, up } => {
            let flags = match (button, up) {
                (MouseButton::Left, false) => MOUSEEVENTF_LEFTDOWN,
                (MouseButton::Left, true) => MOUSEEVENTF_LEFTUP,
                (MouseButton::Right, false) => MOUSEEVENTF_RIGHTDOWN,
                (MouseButton::Right, true) => MOUSEEVENTF_RIGHTUP,
                (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEDOWN,
                (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEUP,
            };
            mouse(0, 0, 0, flags)
        }
        RawInput::Wheel { delta, horizontal } => mouse(
            0,
            0,
            delta,
            if horizontal {
                MOUSEEVENTF_HWHEEL
            } else {
                MOUSEEVENTF_WHEEL
            },
        ),
    }
}

impl InputBackend for WinInputBackend<'_> {
    fn now_ms(&self) -> u64 {
        now_ms()
    }

    fn pause_ms(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }

    fn last_physical_input_ms(&self) -> Option<u64> {
        self.activity.last_physical()
    }

    fn foreground_target(&self) -> Option<TargetWindow> {
        // SAFETY: odczyt okna pierwszego planu.
        target_of(unsafe { GetForegroundWindow() })
    }

    fn target_at(&self, x: i32, y: i32) -> Option<TargetWindow> {
        // SAFETY: zapytanie o okno pod punktem.
        target_of(unsafe { WindowFromPoint(POINT { x, y }) })
    }

    fn focused_field(&self) -> FocusedField {
        self.uia
            .call("element z fokusem", self.uia_ms, |ctx| {
                Ok(uia::focused_field(ctx))
            })
            .unwrap_or(FocusedField::Unknown)
    }

    fn inject(&self, events: &[RawInput]) -> Result<(), GuiError> {
        let inputs: Vec<INPUT> = events.iter().map(convert).collect();
        // SAFETY: tablica `INPUT` o rozmiarze elementu `size_of::<INPUT>()`.
        let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if sent as usize == inputs.len() {
            Ok(())
        } else if sent == 0 {
            Err(last_error("SendInput"))
        } else {
            Err(GuiError::Platform(PlatformError::Io(format!(
                "SendInput: wysłano {sent} z {} zdarzeń paczki",
                inputs.len()
            ))))
        }
    }
}
