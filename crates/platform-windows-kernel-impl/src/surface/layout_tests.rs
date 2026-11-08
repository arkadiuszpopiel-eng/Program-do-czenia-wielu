//! Układ okna Sędziego Cieni na niskim ekranie (przegląd 1.11): przy otwartej pomocy najpierw
//! odpadają wiersze szczegółów, opis pod „?” skraca się dopiero potem; „Co dokładnie” i stopka
//! zostają zawsze.

use windows::Win32::Foundation::RECT;

use super::layout::{Lead, Measured, Row, rect};

/// Okno 600 px: pomoc 200 px (y 100–300), „Co dokładnie” (310–390), 3 wiersze po 40 px (400–520),
/// podpowiedź (530–550), stopka (570–700).
fn measured(help: bool) -> Measured {
    let row = |y: i32| Row {
        label: rect(24, y, 128, 30),
        value: rect(164, y, 400, 30),
        detail: 1,
    };
    Measured {
        client: (600, 700),
        header: rect(0, 0, 600, 48),
        brand: rect(20, 0, 400, 48),
        help_button: Some(rect(520, 6, 36, 36)),
        close_button: rect(558, 6, 36, 36),
        chips: Vec::new(),
        title: rect(24, 60, 552, 30),
        help: help.then(|| rect(24, 100, 552, 200)),
        lead: Some(Lead {
            panel: rect(24, 310, 552, 80),
            label: rect(40, 322, 520, 20),
            value: rect(40, 346, 520, 30),
        }),
        rows: vec![row(400), row(440), row(480)],
        status: rect(24, 530, 552, 20),
        footer: rect(0, 570, 600, 130),
        choice_label: None,
        buttons: vec![(1, rect(24, 590, 140, 40))],
    }
}

fn height(r: &RECT) -> i32 {
    r.bottom - r.top
}

#[test]
fn open_help_keeps_its_text_and_drops_detail_rows_first() {
    let mut m = measured(true);
    m.fit(600, 10);
    assert_eq!(m.client.1, 600);
    assert_eq!(m.help.map(|h| height(&h)), Some(200), "pomoc nieprzycięta");
    assert!(m.rows.len() < 3, "wiersze szczegółów odpadły");
    assert!(m.lead.is_some(), "„Co dokładnie” zostaje");
    let last = m.rows.last().map_or(0, |r| r.value.bottom);
    assert!(last <= m.status.top, "wiersz nie wchodzi pod podpowiedź");
    assert_eq!(m.footer.bottom, 600, "stopka na dole okna");
}

#[test]
fn help_shrinks_only_after_all_rows_are_gone() {
    let mut m = measured(true);
    m.fit(500, 10);
    assert_eq!(m.client.1, 500);
    assert!(m.rows.is_empty());
    let help = m.help.map_or(0, |h| height(&h));
    assert!(
        (60..200).contains(&help),
        "pomoc skrócona, ale nie do jednego wiersza: {help}"
    );
    let lead = m.lead.as_ref().map_or(0, |l| l.panel.bottom);
    assert!(lead <= m.status.top, "„Co dokładnie” nad podpowiedzią");
}

#[test]
fn without_help_rows_drop_as_before() {
    let mut m = measured(false);
    m.fit(640, 10);
    assert_eq!(m.client.1, 640);
    assert!(m.rows.len() < 3);
    assert!(m.lead.is_some());
}
