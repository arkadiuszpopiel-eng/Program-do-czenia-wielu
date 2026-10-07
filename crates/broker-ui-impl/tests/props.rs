//! Własności (ACC-F3-broker-ui-02, logika): dowolny ciąg zdarzeń okna z wejściem wstrzykniętym
//! nigdy nie daje decyzji (= „SendInput do Broker-UI: 0 sukcesów”); każda decyzja w ciągu
//! mieszanym pochodzi z wejścia fizycznego po ≥ 500 ms nieprzerwanego pierwszego planu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use broker_ui_contract::contract_tests::{challenge, decision_matches};
use broker_ui_contract::{BrokerUi, MIN_FOREGROUND_MS, NoHello, UiConfig};
use broker_ui_impl::NativeBrokerUi;
use platform_contract::{InputDevice, InputSample, SurfaceEvent};
use platform_fake::FakeSurface;
use proptest::prelude::*;

const T0: u64 = 1_000_000;

fn device() -> impl Strategy<Value = InputDevice> {
    prop_oneof![
        Just(InputDevice::Keyboard),
        Just(InputDevice::Mouse),
        Just(InputDevice::Touch),
        Just(InputDevice::Pen),
        Just(InputDevice::Unknown),
    ]
}

/// (rodzaj, przycisk, urządzenie, wstrzyknięte, przesunięcie czasu, zasłonięte)
type Raw = (u8, u16, InputDevice, bool, u64, bool);

fn raw(injected: impl Strategy<Value = bool>) -> impl Strategy<Value = Raw> {
    (
        0u8..5,
        99u16..104,
        device(),
        injected,
        0u64..800,
        any::<bool>(),
    )
}

fn event(r: &Raw, at_ms: u64) -> SurfaceEvent {
    let (kind, id, device, injected, _, occluded) = *r;
    let input = InputSample {
        device,
        injected,
        at_ms,
    };
    match kind {
        0 => SurfaceEvent::Activated { at_ms },
        1 => SurfaceEvent::Deactivated { at_ms },
        2 => SurfaceEvent::Cancel { input },
        3 => SurfaceEvent::Closed,
        _ => SurfaceEvent::Button {
            id,
            input,
            occluded,
        },
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn injected_events_never_decide(events in prop::collection::vec(raw(Just(true)), 1..40)) {
        let surface = Arc::new(FakeSurface::new());
        let mut ui = NativeBrokerUi::new(surface.clone(), Arc::new(NoHello), UiConfig::default());
        ui.show(challenge(1, T0), T0).unwrap();
        let mut now = T0;
        prop_assert!(ui.poll_decision(now, 0).is_none());
        for r in &events {
            now += r.4;
            surface.push(event(r, now));
            prop_assert!(ui.poll_decision(now, 0).is_none());
        }
        prop_assert_eq!(ui.queued().len(), 1);
    }

    #[test]
    fn every_decision_is_physical_and_after_500ms(
        events in prop::collection::vec(raw(any::<bool>()), 1..40),
    ) {
        let surface = Arc::new(FakeSurface::new());
        let mut ui = NativeBrokerUi::new(surface.clone(), Arc::new(NoHello), UiConfig::default());
        let ch = challenge(1, T0);
        ui.show(ch.clone(), T0).unwrap();
        let mut now = T0;
        ui.poll_decision(now, 0);
        let mut since: Option<u64> = None;
        let mut decisions = 0;
        for r in &events {
            now += r.4;
            let ev = event(r, now);
            match ev {
                SurfaceEvent::Activated { at_ms } => { since.get_or_insert(at_ms); }
                SurfaceEvent::Deactivated { .. } | SurfaceEvent::Closed => since = None,
                _ => {}
            }
            surface.push(ev);
            if let Some(d) = ui.poll_decision(now, 0) {
                decisions += 1;
                prop_assert!(decision_matches(&d, &ch).is_ok());
                prop_assert!(!d.proof.injected());
                let s = since.expect("decyzja bez pierwszego planu");
                prop_assert!(d.proof.at_ms() >= s + MIN_FOREGROUND_MS);
            }
        }
        prop_assert!(decisions <= 1, "nonce zużyty raz");
    }
}
