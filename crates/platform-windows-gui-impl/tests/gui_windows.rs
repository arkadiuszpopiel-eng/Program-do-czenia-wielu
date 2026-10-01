//! Testy Windows: wywołania bez pulpitu interaktywnego (CI `windows-latest`) oraz `#[ignore]`
//! wymagające pulpitu (self-hosted): Notatnik — drzewo UIA, wpisanie tekstu, odczyt, zrzut,
//! okno; odmowa wobec okna bieżącego procesu (chronionego).

#![cfg(windows)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use platform_contract::{
    CaptureRequest, CaptureTarget, DesktopPort, GuiError, InputControl, InputPlan, InputPort,
    InputStep, ScreenCapturePort, ScreenRect, TreeOptions, UiaPort, UiaQuery, WindowId,
    WindowState,
};
use platform_windows_gui_impl::WinGui;

#[test]
fn queries_do_not_fail_without_desktop() {
    let gui = WinGui::default();
    let windows = gui.windows().unwrap();
    assert!(
        windows
            .iter()
            .all(|w| w.protected == gui.guard().is_protected(w.pid, &w.image))
    );
    let _ = gui.monitors().unwrap();
    let _ = gui.foreground().unwrap();
    assert!(matches!(
        gui.focus(WindowId(0)),
        Err(GuiError::ElementNotFound(_))
    ));
    assert!(matches!(
        gui.set_state(WindowId(0), WindowState::Minimized),
        Err(GuiError::ElementNotFound(_))
    ));
    let bad = CaptureRequest {
        max_width: 1,
        ..CaptureRequest::new(CaptureTarget::Monitor { index: 0 })
    };
    assert!(matches!(gui.capture(&bad), Err(GuiError::Policy(_))));
    // Wątek UIA (COM MTA) startuje; okno 0 nie istnieje → błąd, nie zawieszenie.
    assert!(gui.tree(WindowId(0), &TreeOptions::default()).is_err());
    assert!(gui.password_rects(WindowId(0)).is_err());
}

struct Notepad(Child);

impl Drop for Notepad {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

fn notepad(gui: &WinGui) -> (Notepad, WindowId) {
    let child = Command::new("notepad.exe").spawn().unwrap();
    let guard = Notepad(child);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let found = gui
            .windows()
            .unwrap()
            .into_iter()
            .find(|w| w.image.to_lowercase().ends_with("notepad.exe") && !w.title.is_empty());
        if let Some(w) = found {
            return (guard, w.id);
        }
        assert!(Instant::now() < deadline, "Notatnik nie wystartował");
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
#[ignore = "wymaga pulpitu interaktywnego (self-hosted runner)"]
fn notepad_uia_input_capture_and_window() {
    let gui = WinGui::default();
    let (_np, w) = notepad(&gui);
    gui.focus(w).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    let tree = gui.tree(w, &TreeOptions::default()).unwrap();
    assert!(!tree.is_sparse(), "{tree:?}");
    let plan = InputPlan {
        window: w,
        steps: vec![InputStep::Text {
            text: "Zażółć gęślą jaźń\n".into(),
        }],
    };
    gui.send(&plan, &InputControl::new()).unwrap();
    let edits = gui
        .find(
            w,
            &UiaQuery {
                role: Some("document".into()),
                max_results: 1,
                ..UiaQuery::default()
            },
        )
        .or_else(|_| {
            gui.find(
                w,
                &UiaQuery {
                    role: Some("edit".into()),
                    max_results: 1,
                    ..UiaQuery::default()
                },
            )
        })
        .unwrap();
    let text = gui
        .read_text(&edits[0].element, 1000)
        .map(|t| t.text)
        .unwrap_or_default();
    assert!(text.contains("Zażółć"), "{text}");
    let shot = gui
        .capture(&CaptureRequest::new(CaptureTarget::Window { window: w }))
        .unwrap();
    assert!(shot.png.starts_with(&[0x89, b'P']) && !shot.black_frame);
    gui.set_bounds(w, ScreenRect::from_xywh(50, 50, 700, 500))
        .unwrap();
    gui.set_state(w, WindowState::Minimized).unwrap();
}

#[test]
#[ignore = "wymaga pulpitu interaktywnego (self-hosted runner)"]
fn monitor_capture_masks_and_hook_detects_user() {
    let gui = WinGui::default();
    let shot = gui
        .capture(&CaptureRequest::new(CaptureTarget::Monitor { index: 0 }))
        .unwrap();
    assert!(shot.width > 0 && shot.png.len() > 100);
    let windows = gui.windows().unwrap();
    for p in windows.iter().filter(|w| {
        w.protected && w.state != WindowState::Minimized && shot.source.intersect(&w.rect).is_some()
    }) {
        assert!(
            shot.masked
                .iter()
                .any(|m| m.rect.intersect(&p.rect).is_some()),
            "okno chronione bez maski: {p:?}"
        );
    }
}
