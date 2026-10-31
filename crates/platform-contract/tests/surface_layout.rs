//! Układ okna zatwierdzeń (`SurfaceView::layout`): karta, siatka etykieta/wartość, przyciski w
//! stopce karty, skalowanie z DPI (wzór: `ApprovalCard` z `ui-kit`).

use platform_contract::{SurfaceButton, SurfaceTone, SurfaceView};

fn view() -> SurfaceView {
    SurfaceView {
        title: "Delta prosi o zgodę".into(),
        badge: "⚠ Ryzyko: wysokie".into(),
        tone: SurfaceTone::High,
        details: vec![("Co".into(), "usuń 14 plików".into())],
        status: String::new(),
        buttons: vec![
            SurfaceButton {
                id: 100,
                label: "Odmów".into(),
            },
            SurfaceButton {
                id: 101,
                label: "Zezwól raz".into(),
            },
        ],
        initial_focus: 100,
        take_focus: false,
        help: String::new(),
    }
}

#[test]
fn layout_scales_with_dpi_and_keeps_tab_order() {
    let v = view();
    let l = v.layout(96);
    assert_eq!(l.size.0, 600);
    assert_eq!(
        l.buttons.iter().map(|b| b.0).collect::<Vec<_>>(),
        [100, 101]
    );
    assert!(l.buttons[0].1.0 < l.buttons[1].1.0, "przyciski w rzędzie");
    let row = l.rows[0];
    assert!(l.title.1 < l.badge.1 && l.badge.1 < row.0.1 && row.0.1 < l.status.1);
    assert_eq!(row.0.1, row.1.1, "etykieta i wartość w jednym wierszu");
    assert!(row.0.0 + row.0.2 < row.1.0, "wartość na prawo od etykiety");
    let (cx, cy, cw, ch) = l.card;
    for (_, (bx, by, bw, bh)) in &l.buttons {
        assert!(
            *bx >= cx && bx + bw <= cx + cw && *by >= cy && by + bh <= cy + ch,
            "przyciski w karcie"
        );
    }
    let big = v.layout(192);
    assert_eq!(big.size.0, 1200);
    assert_eq!(big.buttons[0].1.3, 72);
    assert_eq!(v.layout(10).size.0, 600, "DPI poniżej 96 przycinane");
    assert_eq!(SurfaceTone::High.rgb(), (0xB3, 0x26, 0x1E));
}

#[test]
fn long_values_get_more_lines() {
    let mut v = view();
    let short = v.layout(96).rows[0].1.3;
    v.details[0].1 = "x".repeat(200);
    assert!(v.layout(96).rows[0].1.3 > short);
    v.details[0].1 = "a\nb\nc".into();
    assert_eq!(v.layout(96).rows[0].1.3, 3 * short);
}
