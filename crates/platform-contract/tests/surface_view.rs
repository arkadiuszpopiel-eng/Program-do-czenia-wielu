//! Sprawdzanie zawartości okna zatwierdzeń (`SurfaceView::validate`): przyciski, teksty, limity
//! i rząd wyboru (czas zgody „zawsze w tym zakresie”, 31.10). Testy poza `src` (limit rozmiaru
//! crate'a).

use platform_contract::{
    MAX_DETAILS, MAX_TEXT_CHARS, SurfaceButton, SurfaceChoice, SurfaceTone, SurfaceView,
};

fn button(id: u16, label: &str) -> SurfaceButton {
    SurfaceButton {
        id,
        label: label.into(),
    }
}

fn view() -> SurfaceView {
    SurfaceView {
        title: "Delta prosi o zgodę".into(),
        badge: "⚠ Ryzyko: wysokie".into(),
        tone: SurfaceTone::High,
        details: vec![("Co".into(), "usuń 14 plików".into())],
        status: String::new(),
        buttons: vec![button(100, "Odmów"), button(101, "Zezwól raz")],
        initial_focus: 100,
        take_focus: false,
        help: String::new(),
        choice: None,
    }
}

#[test]
fn view_validation() {
    assert!(view().validate().is_ok());
    let mut v = view();
    v.buttons[1].id = 100;
    assert!(v.validate().is_err());
    let mut v = view();
    v.buttons[0].id = 1;
    v.initial_focus = 1;
    assert!(v.validate().is_err(), "IDOK zarezerwowany");
    let mut v = view();
    v.initial_focus = 7;
    assert!(v.validate().is_err());
    let mut v = view();
    v.buttons.clear();
    assert!(v.validate().is_err());
    let mut v = view();
    v.title = "a\u{7}b".into();
    assert!(v.validate().is_err());
    let mut v = view();
    v.details[0].1 = "x".repeat(MAX_TEXT_CHARS + 1);
    assert!(v.validate().is_err());
    let mut v = view();
    v.details = vec![("a".into(), "b".into()); MAX_DETAILS + 1];
    assert!(v.validate().is_err());
    let mut v = view();
    v.details[0].1 = "wiele\nlinii".into();
    assert!(v.validate().is_ok());
}

#[test]
fn choice_row_uses_existing_buttons_other_than_the_safe_one() {
    let with = |buttons: Vec<u16>, label: &str| {
        let mut v = view();
        v.buttons
            .extend([button(102, "1 h"), button(103, "Do odwołania")]);
        v.choice = Some(SurfaceChoice {
            label: label.into(),
            buttons,
        });
        v
    };
    assert!(
        with(vec![102, 103], "Zawsze w tym zakresie:")
            .validate()
            .is_ok()
    );
    assert!(with(vec![], "Zawsze:").validate().is_err(), "pusty rząd");
    assert!(
        with(vec![102, 104], "Zawsze:").validate().is_err(),
        "nieznany przycisk"
    );
    assert!(
        with(vec![100, 102], "Zawsze:").validate().is_err(),
        "„Odmów” w rzędzie"
    );
    assert!(
        with(vec![102], "a\u{7}b").validate().is_err(),
        "etykieta ze sterującym"
    );
}
