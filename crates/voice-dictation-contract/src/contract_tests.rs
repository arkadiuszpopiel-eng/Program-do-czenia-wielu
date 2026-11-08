//! Współdzielone testy kontraktowe `Dictation` (feature `contract-tests`) na wirtualnym pulpicie
//! sterowanym przez [`DesktopDriver`] (atrapa `-fake` albo `platform-fake::FakeDesktop` w `-impl`).

use crate::{Dictation, DictationError, DictationMode, DictationPhase, PauseReason, RefuseReason};

/// Sterowanie wirtualnym pulpitem w teście.
pub trait DesktopDriver {
    /// Nowe okno aplikacji `image` na wierzchu, z fokusem w polu edycji; zwraca id okna.
    fn open_app(&self, image: &str) -> u64;
    /// Okno aplikacji z fokusem w polu hasła.
    fn open_password_app(&self) -> u64;
    /// Okno procesu podniesionego (administratora).
    fn open_elevated_app(&self) -> u64;
    /// Fokus na okno.
    fn focus(&self, window: u64);
    /// Tekst wpisany do okna.
    fn text(&self, window: u64) -> String;
}

fn ok<T>(r: Result<T, DictationError>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

/// Wpisywanie z normalizacją, „cofnij to”, „koniec dyktowania”.
pub fn typing_and_undo<D: Dictation>(d: &mut D, desk: &dyn DesktopDriver) {
    let editor = desk.open_app("notepad.exe");
    ok(d.start(DictationMode::Toggle, 0));
    assert_eq!(d.status().app.as_deref(), Some("notepad.exe"));
    ok(d.on_final("ala ma kota kropka", 100));
    d.tick(110);
    ok(d.on_final("ma dwadzieścia trzy lata", 200));
    d.tick(210);
    assert_eq!(desk.text(editor), "Ala ma kota. Ma 23 lata");
    ok(d.on_final("cofnij to", 300));
    d.tick(310);
    assert_eq!(desk.text(editor), "Ala ma kota.");
    ok(d.on_final("koniec dyktowania", 400));
    assert_eq!(d.status().phase, DictationPhase::Idle);
    assert_eq!(
        d.on_final("po końcu", 500).err(),
        Some(DictationError::NotActive)
    );
    assert_eq!(desk.text(editor), "Ala ma kota.");
}

/// Zmiana okna → pauza (nic do innego okna), powrót → dopisanie.
pub fn focus_change_pauses<D: Dictation>(d: &mut D, desk: &dyn DesktopDriver) {
    let editor = desk.open_app("wordpad.exe");
    ok(d.start(DictationMode::PushToTalk, 0));
    let other = desk.open_app("chrome.exe");
    d.tick(10);
    assert_eq!(
        d.status().phase,
        DictationPhase::Paused(PauseReason::FocusChanged)
    );
    ok(d.on_final("tajna notatka", 20));
    d.tick(30);
    assert_eq!(desk.text(other), "", "nic do okna, które nie było celem");
    assert_eq!(desk.text(editor), "");
    desk.focus(editor);
    d.tick(40);
    assert_eq!(desk.text(editor), "Tajna notatka");
    assert_eq!(desk.text(other), "");
    d.stop();
}

/// Okna chronione, administratora i pola haseł — odmowa, zero wpisów.
pub fn refuses_forbidden_targets<D: Dictation>(d: &mut D, desk: &dyn DesktopDriver) {
    let cases: [(&dyn Fn() -> u64, RefuseReason); 4] = [
        (
            &|| desk.open_app("alfa-desktop.exe"),
            RefuseReason::ProtectedTarget,
        ),
        (
            &|| desk.open_app("alfa-broker-ui.exe"),
            RefuseReason::ProtectedTarget,
        ),
        (&|| desk.open_elevated_app(), RefuseReason::ElevatedTarget),
        (&|| desk.open_password_app(), RefuseReason::PasswordField),
    ];
    for (open, reason) in cases {
        let window = open();
        assert_eq!(
            d.start(DictationMode::Toggle, 0).err(),
            Some(DictationError::Refused(reason))
        );
        assert_eq!(d.status().phase, DictationPhase::Idle);
        assert!(d.on_final("tekst", 10).is_err());
        assert_eq!(desk.text(window), "");
    }
    let events = d.take_events();
    assert!(
        events
            .iter()
            .filter(|e| e.name() == crate::EVENT_REFUSED)
            .count()
            >= 4
    );
    for e in events {
        assert!(!e.to_bus_event().payload.to_string().contains("tekst"));
    }
}

/// Cały zestaw na świeżych instancjach z `factory`.
pub fn run_all<D: Dictation, F: Fn() -> (D, Box<dyn DesktopDriver>)>(factory: F) {
    let (mut d, desk) = factory();
    typing_and_undo(&mut d, desk.as_ref());
    let (mut d, desk) = factory();
    focus_change_pauses(&mut d, desk.as_ref());
    let (mut d, desk) = factory();
    refuses_forbidden_targets(&mut d, desk.as_ref());
}
