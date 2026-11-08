//! Reguły dowodu: wstrzyknięte wejście nigdy nie przechodzi (własność), clickjacking (okno
//! krócej niż 500 ms na pierwszym planie), nakładka, wejście spoza okna ważności.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use broker_ui_contract::{ForegroundTracker, MIN_FOREGROUND_MS, RejectReason, check_input};
use platform_contract::{InputDevice, InputSample};
use proptest::prelude::*;
use safety_broker_contract::InputSource;

fn sample(device: InputDevice, injected: bool, at_ms: u64) -> InputSample {
    InputSample {
        device,
        injected,
        at_ms,
    }
}

#[test]
fn clickjacking_and_window_rules() {
    let mut fg = ForegroundTracker::default();
    let ok = sample(InputDevice::Mouse, false, 2_000);
    assert_eq!(
        check_input(&fg, &ok, false, 1_000, 9_000),
        Err(RejectReason::NotForeground)
    );
    fg.activated(1_600);
    assert_eq!(
        check_input(&fg, &ok, false, 1_000, 9_000),
        Err(RejectReason::TooSoon { elapsed_ms: 400 })
    );
    fg.activated(1_900);
    assert_eq!(
        fg.since(),
        Some(1_600),
        "ponowna aktywacja nie przesuwa początku"
    );
    let later = sample(InputDevice::Mouse, false, 2_100);
    assert_eq!(
        check_input(&fg, &later, false, 1_000, 9_000),
        Ok(InputSource::MouseClick)
    );
    assert_eq!(
        check_input(&fg, &later, true, 1_000, 9_000),
        Err(RejectReason::Occluded)
    );
    let kb = sample(InputDevice::Keyboard, false, 2_100);
    assert_eq!(
        check_input(&fg, &kb, false, 1_000, 9_000),
        Ok(InputSource::Keyboard)
    );
    let unknown = sample(InputDevice::Unknown, false, 2_100);
    assert_eq!(
        check_input(&fg, &unknown, false, 1_000, 9_000),
        Err(RejectReason::UnknownDevice)
    );
    assert_eq!(
        check_input(&fg, &later, false, 2_200, 9_000),
        Err(RejectReason::BeforeShown)
    );
    assert_eq!(
        check_input(&fg, &later, false, 1_000, 2_100),
        Err(RejectReason::Expired)
    );
    fg.deactivated();
    assert_eq!(
        check_input(&fg, &later, false, 1_000, 9_000),
        Err(RejectReason::NotForeground)
    );
    let touch = sample(InputDevice::Touch, false, 3_000);
    fg.activated(2_000);
    assert_eq!(
        check_input(&fg, &touch, false, 1_000, 9_000),
        Ok(InputSource::MouseClick)
    );
    assert!(
        RejectReason::TooSoon { elapsed_ms: 3 }
            .to_string()
            .contains("kliknij ponownie")
    );
}

fn device() -> impl Strategy<Value = InputDevice> {
    prop_oneof![
        Just(InputDevice::Keyboard),
        Just(InputDevice::Mouse),
        Just(InputDevice::Touch),
        Just(InputDevice::Pen),
        Just(InputDevice::Unknown),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(3000))]

    /// „SendInput do Broker-UI = 0 sukcesów” (logika): wejście wstrzyknięte nigdy nie jest dowodem,
    /// niezależnie od urządzenia, czasu, pierwszego planu i zasłonięcia.
    #[test]
    fn injected_input_never_accepted(
        dev in device(), at in 0u64..1_000_000, since in proptest::option::of(0u64..1_000_000),
        occluded in any::<bool>(), shown in 0u64..1_000_000, ttl in 1u64..1_000_000,
    ) {
        let mut fg = ForegroundTracker::default();
        if let Some(s) = since { fg.activated(s); }
        let r = check_input(&fg, &sample(dev, true, at), occluded, shown, shown + ttl);
        prop_assert_eq!(r, Err(RejectReason::Injected));
    }

    /// Każde przyjęte wejście: niewstrzyknięte, okno ≥ 500 ms na pierwszym planie, niezasłonięte,
    /// w oknie ważności karty.
    #[test]
    fn accepted_input_satisfies_all_rules(
        dev in device(), injected in any::<bool>(), at in 0u64..100_000,
        since in proptest::option::of(0u64..100_000), occluded in any::<bool>(),
        shown in 0u64..100_000, ttl in 1u64..100_000,
    ) {
        let mut fg = ForegroundTracker::default();
        if let Some(s) = since { fg.activated(s); }
        if check_input(&fg, &sample(dev, injected, at), occluded, shown, shown + ttl).is_ok() {
            let s = since.unwrap();
            prop_assert!(!injected && !occluded && dev != InputDevice::Unknown);
            prop_assert!(at >= s + MIN_FOREGROUND_MS && at >= shown && at < shown + ttl);
        }
    }
}
