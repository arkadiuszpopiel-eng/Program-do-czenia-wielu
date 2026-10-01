//! Współdzielone testy kontraktowe `Wake` (feature `contract-tests`).
//! `press`/`release` symulują hook klawiatury (atrapa portu skrótów).

use personas_contract::PersonaId;

use crate::{MicState, Wake, WakeCfg, WakeEvent, WakeInput, WakeSource};

/// Sterowanie klawiszami w teście (wciśnięcie/puszczenie PTT i przełącznika).
pub trait KeyDriver {
    /// Wciska (`true`) / puszcza PTT.
    fn ptt(&self, pressed: bool);
    /// Wciska przełącznik.
    fn toggle(&self);
}

/// PTT: wciśnięcie → słucha; puszczenie → koniec; stan mikrofonu jako zdarzenia.
pub fn ptt_cycle<W: Wake>(wake: &mut W, keys: &dyn KeyDriver) {
    wake.configure(WakeCfg::default())
        .unwrap_or_else(|e| panic!("{e}"));
    keys.ptt(true);
    let ev = wake.pump();
    assert!(
        ev.contains(&WakeEvent::ListenStart {
            addressed: None,
            source: WakeSource::Ptt
        }),
        "{ev:?}"
    );
    assert!(wake.is_listening());
    assert_eq!(wake.mic_state(), MicState::Listening);
    keys.ptt(false);
    let ev = wake.pump();
    assert!(
        ev.contains(&WakeEvent::ListenStop {
            source: WakeSource::Ptt
        }),
        "{ev:?}"
    );
    assert!(!wake.is_listening());
    assert_eq!(wake.mic_state(), MicState::Off);
}

/// Przełącznik włącza i wyłącza słuchanie.
pub fn toggle_cycle<W: Wake>(wake: &mut W, keys: &dyn KeyDriver) {
    wake.configure(WakeCfg::default())
        .unwrap_or_else(|e| panic!("{e}"));
    keys.toggle();
    wake.pump();
    assert!(wake.is_listening());
    keys.toggle();
    wake.pump();
    assert!(!wake.is_listening());
}

/// Adresowanie: imię wygrywa.
pub fn addressing<W: Wake>(wake: &mut W) {
    assert_eq!(
        wake.addressed("Beta, co masz na dziś?"),
        Some(PersonaId::beta())
    );
    let ev = wake.handle(WakeInput::Transcript {
        text: "Delto, zrób kopię".into(),
    });
    assert!(ev.contains(&WakeEvent::Addressed {
        persona: PersonaId::delta(),
        by_name: true
    }));
}

/// Niepoprawna konfiguracja (v1) odrzucona.
pub fn rejects_v1<W: Wake>(wake: &mut W) {
    let cfg = WakeCfg {
        wake_words: Some(crate::WakeWordCfg {
            phrases: vec![],
            threshold: 0.5,
            always_on: true,
            owner_gate: false,
        }),
        ..WakeCfg::default()
    };
    assert!(wake.configure(cfg).is_err());
}

/// Cały zestaw; `factory` daje świeżą instancję i sterownik klawiszy.
pub fn run_all<W: Wake, F: Fn() -> (W, Box<dyn KeyDriver>)>(factory: F) {
    let (mut w, k) = factory();
    ptt_cycle(&mut w, k.as_ref());
    let (mut w, k) = factory();
    toggle_cycle(&mut w, k.as_ref());
    let (mut w, _) = factory();
    addressing(&mut w);
    let (mut w, _) = factory();
    rejects_v1(&mut w);
}
