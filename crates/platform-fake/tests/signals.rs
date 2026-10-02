//! Sygnały systemowe na atrapie z wirtualnym zegarem (ten sam `SignalMonitor` co monitor
//! Windows): bezczynność z histerezą (trącenie myszy nie budzi), zasilanie bez zdarzenia na każdy
//! 1%, tryb gry (wejście od razu, wyjście po 10 s), blokada sesji, odczyty nieudane bez zgadywania,
//! ograniczona kolejka.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use platform_contract::{
    EVENT_FULLSCREEN_CHANGED, EVENT_IDLE_ENTERED, EVENT_IDLE_EXITED, EVENT_SESSION_LOCKED,
    EVENT_SESSION_UNLOCKED, FullscreenPort, FullscreenProbe, GameReason, IdleConfig, IdlePort,
    MAX_QUEUED_SIGNALS, NotificationState, PowerPort, PowerSnapshot, SessionPort, SessionState,
    SignalConfig, SignalEvent, SystemSignalsPort,
};
use platform_fake::FakeSignals;

const MIN: u64 = 60_000;

fn fake() -> FakeSignals {
    let s = FakeSignals::default();
    s.advance(0);
    // Pierwsza próbka zgłasza zasilanie.
    assert_eq!(
        s.drain_events(),
        vec![SignalEvent::PowerChanged {
            power: PowerSnapshot::AC
        }]
    );
    s
}

fn busy() -> FullscreenProbe {
    FullscreenProbe {
        notification: NotificationState::Busy,
        foreground_fullscreen: true,
        foreground_image: Some("gra.exe".into()),
    }
}

#[test]
fn idle_enters_after_threshold_and_ignores_a_single_bump() {
    let s = fake();
    s.advance(4 * MIN);
    assert!(s.drain_events().is_empty());
    assert!(!s.snapshot().user_idle);
    s.advance(MIN);
    let ev = s.drain_events();
    assert_eq!(ev, vec![SignalEvent::IdleEntered { idle_ms: 5 * MIN }]);
    assert_eq!(ev[0].name(), EVENT_IDLE_ENTERED);
    assert!(s.snapshot().user_idle);
    assert_eq!(s.snapshot().idle_secs(), 300);
    // Trącenie myszy: jedno wejście, potem cisza — bez wyjścia z bezczynności.
    s.input();
    s.advance(30_000);
    assert!(s.drain_events().is_empty());
    assert!(s.snapshot().user_idle);
    // Drugie trącenie po oknie potwierdzenia (10 s) — też nic.
    s.input();
    s.advance(15_000);
    assert!(s.drain_events().is_empty());
    // Powrót: ruch trwa ≥ 1 s.
    s.input();
    s.advance(600);
    s.input();
    s.advance(600);
    s.input();
    s.advance(600);
    s.input();
    s.advance(1_000);
    let ev = s.drain_events();
    assert_eq!(ev.len(), 1);
    assert!(matches!(ev[0], SignalEvent::IdleExited { idle_for_ms } if idle_for_ms >= 5 * MIN));
    assert_eq!(ev[0].name(), EVENT_IDLE_EXITED);
    assert!(!s.snapshot().user_idle);
    // Ponowne wejście wymaga pełnego progu.
    s.advance(4 * MIN);
    assert!(s.drain_events().is_empty());
    s.advance(MIN + 2_000);
    assert!(matches!(
        s.drain_events()[..],
        [SignalEvent::IdleEntered { .. }]
    ));
}

#[test]
fn immediate_wake_configuration() {
    let s = FakeSignals::new(SignalConfig {
        idle: IdleConfig {
            idle_after_ms: 2_000,
            wake_confirm_ms: 0,
            wake_window_ms: 0,
        },
        ..SignalConfig::default()
    });
    s.advance(2_000);
    assert!(s.snapshot().user_idle);
    s.input();
    s.advance(1_000);
    assert!(!s.snapshot().user_idle);
    assert!(
        s.drain_events()
            .iter()
            .any(|e| matches!(e, SignalEvent::IdleExited { .. }))
    );
}

#[test]
fn unreadable_idle_counter_changes_nothing() {
    let s = fake();
    s.fail_idle(true);
    assert!(s.idle_ms().is_err());
    s.advance(10 * MIN);
    assert!(s.drain_events().is_empty());
    assert!(!s.snapshot().user_idle, "brak odczytu ≠ bezczynność");
    s.fail_idle(false);
    assert_eq!(s.idle_ms().unwrap(), 10 * MIN);
    s.advance(1_000);
    assert!(s.snapshot().user_idle);
}

#[test]
fn power_changes_are_filtered() {
    let s = fake();
    s.set_power(Some(PowerSnapshot::battery(60)));
    s.notify();
    assert_eq!(
        s.drain_events(),
        vec![SignalEvent::PowerChanged {
            power: PowerSnapshot::battery(60)
        }]
    );
    assert!(s.snapshot().on_battery());
    for p in [59, 58, 57, 56] {
        s.set_power(Some(PowerSnapshot::battery(p)));
        s.advance(1_000);
    }
    assert!(s.drain_events().is_empty());
    assert_eq!(s.snapshot().power.battery_percent, Some(56));
    s.set_power(Some(PowerSnapshot::battery(55)));
    s.advance(1_000);
    assert_eq!(s.drain_events().len(), 1);
    s.set_power(Some(PowerSnapshot {
        saver: true,
        ..PowerSnapshot::battery(55)
    }));
    s.advance(1_000);
    assert_eq!(s.drain_events().len(), 1);
    s.set_power(None);
    assert!(s.power().is_err());
    s.advance(5_000);
    assert!(s.drain_events().is_empty());
    assert!(s.snapshot().on_battery(), "brak odczytu nie zmienia stanu");
    s.set_power(Some(PowerSnapshot::AC));
    s.advance(1_000);
    assert_eq!(
        s.drain_events(),
        vec![SignalEvent::PowerChanged {
            power: PowerSnapshot::AC
        }]
    );
}

#[test]
fn game_mode_enters_at_once_and_leaves_after_ten_seconds() {
    let s = fake();
    s.set_fullscreen(Some(busy()));
    s.advance(1_000);
    let ev = s.drain_events();
    assert_eq!(
        ev,
        vec![SignalEvent::GameModeChanged {
            reason: Some(GameReason::Busy)
        }]
    );
    assert_eq!(ev[0].name(), EVENT_FULLSCREEN_CHANGED);
    assert!(s.snapshot().game_mode());
    assert_eq!(s.probe().unwrap().game_reason(), Some(GameReason::Busy));
    // Alt-tab na 5 s — tryb gry trwa.
    s.set_fullscreen(Some(FullscreenProbe::none()));
    s.advance(5_000);
    s.set_fullscreen(Some(busy()));
    s.advance(1_000);
    assert!(s.drain_events().is_empty());
    // Odczyt nieudany nie kończy trybu gry.
    s.set_fullscreen(None);
    s.advance(60_000);
    assert!(s.drain_events().is_empty());
    assert!(s.snapshot().game_mode());
    s.set_fullscreen(Some(FullscreenProbe::none()));
    s.advance(9_000);
    assert!(s.drain_events().is_empty());
    s.advance(2_000);
    assert_eq!(
        s.drain_events(),
        vec![SignalEvent::GameModeChanged { reason: None }]
    );
    // Wygaszacz/blokada (`NotPresent`) z oknem na cały ekran to nie gra.
    s.set_fullscreen(Some(FullscreenProbe {
        notification: NotificationState::NotPresent,
        foreground_fullscreen: true,
        foreground_image: None,
    }));
    s.advance(3_000);
    assert!(!s.snapshot().game_mode());
}

#[test]
fn session_lock_and_unlock() {
    let s = fake();
    s.set_session(Some(SessionState::Locked));
    s.notify();
    let ev = s.drain_events();
    assert_eq!(
        ev,
        vec![SignalEvent::SessionChanged {
            state: SessionState::Locked
        }]
    );
    assert_eq!(ev[0].name(), EVENT_SESSION_LOCKED);
    assert!(s.snapshot().locked());
    assert_eq!(s.session().unwrap(), SessionState::Locked);
    // Stan nieznany i błąd odczytu nie odblokowują.
    s.set_session(Some(SessionState::Unknown));
    s.advance(2_000);
    s.set_session(None);
    s.advance(2_000);
    assert!(s.drain_events().is_empty());
    assert!(s.snapshot().locked());
    s.set_session(Some(SessionState::Disconnected));
    s.advance(1_000);
    let ev = s.drain_events();
    assert_eq!(ev[0].name(), EVENT_SESSION_LOCKED);
    s.set_session(Some(SessionState::Active));
    s.advance(1_000);
    let ev = s.drain_events();
    assert_eq!(ev[0].name(), EVENT_SESSION_UNLOCKED);
    assert!(!s.snapshot().locked());
}

#[test]
fn queue_is_bounded_and_wait_uses_virtual_time() {
    let s = fake();
    for i in 0..(MAX_QUEUED_SIGNALS + 10) {
        let state = if i % 2 == 0 {
            SessionState::Locked
        } else {
            SessionState::Active
        };
        s.set_session(Some(state));
        s.notify();
    }
    assert_eq!(s.drain_events().len(), MAX_QUEUED_SIGNALS);
    assert_eq!(s.dropped(), 10);
    let start = s.now_ms();
    let ev = s.wait_events(Duration::from_secs(600));
    assert_eq!(ev, vec![SignalEvent::IdleEntered { idle_ms: 5 * MIN }]);
    assert_eq!(s.now_ms() - start, 5 * MIN);
    assert!(s.wait_events(Duration::from_secs(3)).is_empty());
    assert_eq!(s.now_ms() - start, 5 * MIN + 3_000);
}
