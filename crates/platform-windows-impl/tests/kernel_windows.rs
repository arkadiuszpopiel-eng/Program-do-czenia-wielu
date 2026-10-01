//! Okno Broker-UI z prawdziwym `SendInput` (ACC-F3-broker-ui-02, część sprzętowa) — wymaga
//! interaktywnego pulpitu, więc `#[ignore]` (self-hosted). Pozostałe testy portów Jądra na
//! Windows (potok z DACL, tożsamość, katalog prywatny, MMCSS, dysk, usługa) są w
//! `crates/app-safety/tests/windows_ports.rs` (limit rozmiaru tego crate'a).

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used, unsafe_code)]

use std::time::{Duration, Instant};

use platform_contract::{
    ApprovalSurfacePort, SurfaceButton, SurfaceEvent, SurfaceTone, SurfaceView,
};
use platform_windows_impl::WinApprovalSurface;
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT, SendInput, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetDlgItem, GetWindowRect, SetCursorPos,
};
use windows::core::w;

fn button(id: u16, label: &str) -> SurfaceButton {
    SurfaceButton {
        id,
        label: label.into(),
    }
}

fn send(inputs: &[INPUT]) {
    // SAFETY: tablica struktur INPUT o poprawnym rozmiarze (syntetyczne wejście testu).
    unsafe { SendInput(inputs, i32::try_from(size_of::<INPUT>()).unwrap()) };
}

fn click_at(x: i32, y: i32) {
    // SAFETY: przesunięcie kursora (test).
    let _ = unsafe { SetCursorPos(x, y) };
    let mouse = |flags| INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    send(&[mouse(MOUSEEVENTF_LEFTDOWN), mouse(MOUSEEVENTF_LEFTUP)]);
}

fn press_space() {
    let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_SPACE,
                dwFlags: flags,
                ..Default::default()
            },
        },
    };
    send(&[key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)]);
}

/// Środek przycisku `id` okna Broker-UI na ekranie.
fn center(id: i32) -> (i32, i32) {
    let mut r = RECT::default();
    // SAFETY: zapytania o okno klasy Broker-UI i jego kontrolkę (tylko odczyt).
    unsafe {
        let window = FindWindowW(w!("AlfaBrokerUi"), None).unwrap();
        GetWindowRect(GetDlgItem(Some(window), id).unwrap(), &raw mut r).unwrap();
    }
    ((r.left + r.right) / 2, (r.top + r.bottom) / 2)
}

fn next_press(s: &WinApprovalSurface) -> Option<SurfaceEvent> {
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if let Some(e @ (SurfaceEvent::Button { .. } | SurfaceEvent::Cancel { .. })) =
            s.next_event(100)
        {
            return Some(e);
        }
    }
    None
}

/// 100 kliknięć `SendInput` w „Zezwól” + 20 × Spacja: każde zdarzenie oznaczone jako
/// wstrzyknięte; logika `broker-ui` (własność w `broker-ui-impl/tests/props.rs`) nigdy nie robi
/// z niego dowodu ⇒ 0 sukcesów.
#[test]
#[ignore = "self-hosted: wymaga interaktywnego pulpitu"]
fn send_input_into_broker_window_is_always_flagged_injected() {
    let s = WinApprovalSurface::new();
    let view = SurfaceView {
        title: "Test: Delta prosi o zgodę & coś".into(),
        badge: "⚠ Ryzyko: wysokie".into(),
        tone: SurfaceTone::High,
        details: vec![("Co".into(), "usuń 14 plików".into())],
        status: String::new(),
        buttons: vec![
            button(100, "Odmów (Esc)"),
            button(101, "Zezwól tylko teraz"),
        ],
        initial_focus: 100,
        take_focus: true,
    };
    s.present(&view).unwrap();
    std::thread::sleep(Duration::from_millis(800));
    let (x, y) = center(101);
    for _ in 0..100 {
        click_at(x, y);
        let Some(SurfaceEvent::Button { input, .. }) = next_press(&s) else {
            panic!("brak zdarzenia przycisku");
        };
        assert!(
            input.injected,
            "kliknięcie SendInput uznane za fizyczne: {input:?}"
        );
    }
    for _ in 0..20 {
        press_space();
        if let Some(SurfaceEvent::Button { input, .. } | SurfaceEvent::Cancel { input }) =
            next_press(&s)
        {
            assert!(
                input.injected,
                "klawisz SendInput uznany za fizyczny: {input:?}"
            );
        }
    }
    s.dismiss().unwrap();
}
