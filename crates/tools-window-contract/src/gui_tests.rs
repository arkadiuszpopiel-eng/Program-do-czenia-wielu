use platform_contract::ScreenRect;
use platform_fake::{FakeDesktop, FakeWindow};
use tools_common_contract::ToolStatus;

use super::*;

#[test]
fn errors_map_to_outcomes_for_the_model() {
    let interrupted = gui_outcome(&GuiError::UserInterrupted { sent: 3 }, "pisanie");
    assert_eq!(interrupted.status, ToolStatus::Cancelled);
    assert!(
        interrupted
            .text
            .contains("fizyczne wejście ma pierwszeństwo")
    );
    assert_eq!(interrupted.data["interrupted_by_user"], true);
    assert_eq!(
        gui_outcome(&GuiError::UserActive, "x").status,
        ToolStatus::Cancelled
    );
    assert_eq!(
        gui_outcome(&GuiError::Cancelled, "x").status,
        ToolStatus::Cancelled
    );
    let protected = gui_outcome(&GuiError::ProtectedTarget("alfa".into()), "klik");
    assert!(matches!(
        protected.status,
        ToolStatus::Denied {
            reason: DenialReason::KernelBlock {
                rule: KernelRule::GuiControlOfKernelProcess
            }
        }
    ));
    let policy = gui_outcome(&GuiError::Policy("skrót Win+R".into()), "skrót");
    assert!(policy.text.contains("Win+R"));
    for (e, kind) in [
        (
            GuiError::Timeout {
                op: "x".into(),
                ms: 5,
            },
            ToolErrorKind::Timeout,
        ),
        (
            GuiError::ElementNotFound("e".into()),
            ToolErrorKind::NotFound,
        ),
        (
            GuiError::PatternUnsupported("p".into()),
            ToolErrorKind::Unsupported,
        ),
        (GuiError::Elevated, ToolErrorKind::Unsupported),
        (
            GuiError::TargetChanged {
                expected: 1,
                actual: None,
            },
            ToolErrorKind::Io,
        ),
        (
            GuiError::Platform(PlatformError::Unsupported("u".into())),
            ToolErrorKind::Unsupported,
        ),
        (
            GuiError::Platform(PlatformError::Io("i".into())),
            ToolErrorKind::Io,
        ),
    ] {
        assert_eq!(
            gui_outcome(&e, "akcja").status,
            ToolStatus::Failed { error: kind },
            "{e:?}"
        );
    }
}

#[test]
fn targets_and_capabilities() {
    let d = FakeDesktop::new();
    let np = d.add_window(
        FakeWindow::new(
            "Notatnik",
            r"C:\Windows\notepad.exe",
            ScreenRect::from_xywh(0, 0, 300, 200),
        ),
        true,
    );
    let alfa = d.add_window(
        FakeWindow::new(
            "Alfa",
            "alfa-desktop.exe",
            ScreenRect::from_xywh(0, 0, 300, 200),
        ),
        false,
    );
    let w = target_window(&d, np, "fokus").unwrap();
    assert_eq!(
        app_capability(&w, "fokus").unwrap().to_string(),
        "gui.control(notepad.exe)"
    );
    assert_eq!(brief(&w).app, "notepad.exe");
    assert!(brief(&w).focused);
    let denied = target_window(&d, alfa, "fokus").unwrap_err();
    assert!(matches!(denied.status, ToolStatus::Denied { .. }));
    assert!(target_window(&d, WindowId(999), "fokus").is_err());
    assert_eq!(
        desktop_capability().unwrap().to_string(),
        "gui.control(desktop.exe)"
    );
}
