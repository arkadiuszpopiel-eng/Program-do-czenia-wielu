use personas_contract::{Catalog, PersonaId, builtin_personas};
use platform_contract::{HotkeyId, KILL_SWITCH};

use super::*;

fn machine() -> WakeMachine {
    let catalog = Catalog::builtin();
    let mut m = WakeMachine::new(builtin_personas(), Some(catalog.default_cast(true)));
    m.set_keys(Some(HotkeyId(1)), Some(HotkeyId(2)));
    m
}

fn key(id: u32, pressed: bool) -> WakeInput {
    WakeInput::Key {
        id: HotkeyId(id),
        pressed,
    }
}

#[test]
fn ptt_press_release_and_autorepeat() {
    let mut m = machine();
    let ev = m.handle(key(1, true));
    assert_eq!(
        ev[0],
        WakeEvent::ListenStart {
            addressed: None,
            source: WakeSource::Ptt
        }
    );
    assert_eq!(
        ev[1],
        WakeEvent::MicState {
            state: MicState::Listening
        }
    );
    assert!(
        m.handle(key(1, true)).is_empty(),
        "autopowtórzenie klawisza"
    );
    assert_eq!(
        m.handle(WakeInput::Vad { speech: true }),
        vec![WakeEvent::MicState {
            state: MicState::Hearing
        }]
    );
    m.handle(WakeInput::Processing { busy: true });
    let ev = m.handle(key(1, false));
    assert_eq!(
        ev[0],
        WakeEvent::ListenStop {
            source: WakeSource::Ptt
        }
    );
    assert_eq!(m.mic_state(), MicState::Processing);
    m.handle(WakeInput::Processing { busy: false });
    assert_eq!(m.mic_state(), MicState::Off);
    assert!(m.handle(key(1, false)).is_empty());
    assert!(m.handle(key(9, true)).is_empty(), "obcy skrót");
}

#[test]
fn toggle_ui_mute_and_dnd() {
    let mut m = machine();
    m.handle(key(2, true));
    assert_eq!(m.listening(), Some(WakeSource::Toggle));
    assert!(
        m.handle(key(2, false)).is_empty(),
        "puszczenie przełącznika nic nie robi"
    );
    m.handle(key(1, true));
    assert_eq!(
        m.listening(),
        Some(WakeSource::Toggle),
        "PTT nie przejmuje otwartego słuchania"
    );
    m.handle(key(1, false));
    assert_eq!(m.listening(), Some(WakeSource::Toggle));
    m.handle(key(2, true));
    assert_eq!(m.listening(), None);
    m.handle(WakeInput::UiToggle);
    assert_eq!(m.listening(), Some(WakeSource::Ui));
    let ev = m.handle(WakeInput::SetMuted { muted: true });
    assert!(ev.contains(&WakeEvent::ListenStop {
        source: WakeSource::Ui
    }));
    assert_eq!(m.mic_state(), MicState::Muted);
    assert!(
        m.handle(WakeInput::UiPtt { pressed: true }).is_empty(),
        "wyciszony"
    );
    m.handle(WakeInput::SetMuted { muted: false });
    assert_eq!(
        m.handle(WakeInput::SetDnd { on: true }),
        vec![WakeEvent::Dnd { on: true }]
    );
    assert!(m.handle(WakeInput::SetDnd { on: true }).is_empty());
    assert!(m.dnd());
    let ev = m.handle(WakeInput::UiPtt { pressed: true });
    assert!(
        matches!(
            ev[0],
            WakeEvent::ListenStart {
                source: WakeSource::Ui,
                ..
            }
        ),
        "DND nie wyłącza PTT"
    );
    m.handle(WakeInput::UiPtt { pressed: false });
    assert_eq!(m.mic_state(), MicState::Off);
}

#[test]
fn addressing_by_name_wins_else_conductor() {
    let mut m = machine();
    let ev = m.handle(WakeInput::Transcript {
        text: "Delto, otwórz plik".into(),
    });
    assert_eq!(
        ev,
        vec![WakeEvent::Addressed {
            persona: PersonaId::delta(),
            by_name: true
        }]
    );
    let ev = m.handle(WakeInput::Transcript {
        text: "jaka jest pogoda?".into(),
    });
    match &ev[0] {
        WakeEvent::Addressed { by_name, persona } => {
            assert!(!by_name);
            assert_eq!(Some(persona), m.last_addressed());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(m.addressee("Hej Gama, sprawdź"), Some(PersonaId::gama()));
    m.set_name_addressing(false);
    assert!(
        m.handle(WakeInput::Transcript {
            text: "Beta, co tam".into()
        })
        .is_empty()
    );
    m.set_cast(None);
    assert_eq!(m.addressee("jaka pogoda"), None);
}

#[test]
fn elevated_foreground_reported_once() {
    let mut m = machine();
    assert_eq!(
        m.handle(WakeInput::ElevatedForeground { elevated: true }),
        vec![WakeEvent::BlockedElevatedForeground]
    );
    assert!(
        m.handle(WakeInput::ElevatedForeground { elevated: true })
            .is_empty()
    );
    m.handle(WakeInput::ElevatedForeground { elevated: false });
    assert_eq!(
        m.handle(WakeInput::ElevatedForeground { elevated: true })
            .len(),
        1
    );
}

#[test]
fn config_validation_and_events() {
    assert!(WakeCfg::default().validate().is_ok());
    let same = WakeCfg {
        toggle_key: WakeCfg::default().ptt_key,
        ..WakeCfg::default()
    };
    assert!(matches!(same.validate(), Err(WakeError::InvalidConfig(_))));
    let kill = WakeCfg {
        ptt_key: Some(KILL_SWITCH),
        ..WakeCfg::default()
    };
    assert!(matches!(kill.validate(), Err(WakeError::Hotkey(_))));
    let v1 = WakeCfg {
        wake_words: Some(WakeWordCfg {
            phrases: vec![],
            threshold: 0.5,
            always_on: false,
            owner_gate: true,
        }),
        ..WakeCfg::default()
    };
    assert!(matches!(v1.validate(), Err(WakeError::NotAvailable(_))));
    let all = [
        WakeEvent::ListenStart {
            addressed: None,
            source: WakeSource::Ptt,
        },
        WakeEvent::ListenStop {
            source: WakeSource::Toggle,
        },
        WakeEvent::Addressed {
            persona: PersonaId::alfa(),
            by_name: true,
        },
        WakeEvent::BlockedElevatedForeground,
        WakeEvent::Dnd { on: true },
        WakeEvent::MicState {
            state: MicState::Hearing,
        },
    ];
    for e in all {
        assert_eq!(e.to_bus_event().kind.as_str(), e.name());
    }
    assert!(event_schema().is_object());
    assert!(!EVENT_FALSE_ALARM.is_empty());
}
