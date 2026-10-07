//! Przegląd Q-9: sterownik Broker-UI nie gubi odmowy właściciela. Błąd łącza przy odmowie (stan
//! prośby nieznany) wraca do wołającego, a karta pojawia się ponownie przy następnej
//! synchronizacji; jawne odrzucenie przez Brokera (`UiError::Rejected`) kończy kartę.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use broker_ui_contract::{BrokerLink, BrokerUi, NoHello, UiConfig, UiDecision, UiError, UiEvent};
use broker_ui_impl::driver::cycle;
use broker_ui_impl::{BTN_DENY, ChannelLink, NativeBrokerUi};
use compliance_contract::PathEnv;
use platform_contract::{InputDevice, InputSample, SurfaceEvent};
use platform_fake::FakeSurface;
use safety_broker_contract::contract_tests::{PROFILE, delta, host, request, test_policy};
use safety_broker_contract::{
    ApprovalChallenge, ApprovalStatus, Broker, Capability, CommandOrigin, Decision,
};
use safety_broker_fake::FakeBroker;
use watchdog_contract::{Clock, ManualClock};

/// Łącze z wstrzykniętą awarią `resolve` (raz).
struct Flaky {
    inner: ChannelLink<FakeBroker>,
    fail_with: Option<UiError>,
}

impl BrokerLink for Flaky {
    fn pending(&mut self) -> Result<Vec<ApprovalChallenge>, UiError> {
        self.inner.pending()
    }

    fn resolve(&mut self, d: UiDecision) -> Result<(), UiError> {
        match self.fail_with.take() {
            Some(e) => Err(e),
            None => self.inner.resolve(d),
        }
    }
}

struct Rig {
    broker: Arc<FakeBroker>,
    clock: Arc<ManualClock>,
    surface: Arc<FakeSurface>,
    ui: NativeBrokerUi<FakeSurface>,
    link: Flaky,
}

fn rig(fail_with: UiError) -> (Rig, safety_broker_contract::ApprovalId) {
    let clock = Arc::new(ManualClock::new(1_000_000));
    let broker = Arc::new(
        FakeBroker::with(
            test_policy(),
            PathEnv::windows_profile(PROFILE),
            clock.clone(),
        )
        .unwrap(),
    );
    let surface = Arc::new(FakeSurface::new());
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let req = request(
        &delta(),
        Capability::NetEgress(host("x.example.org")),
        CommandOrigin::UserText,
    );
    let Decision::NeedsApproval(ticket) = rt.block_on(broker.decide(req)).unwrap() else {
        panic!("oczekiwano prośby");
    };
    let rig = Rig {
        ui: NativeBrokerUi::new(surface.clone(), Arc::new(NoHello), UiConfig::default()),
        link: Flaky {
            inner: ChannelLink::new(broker.clone()).unwrap(),
            fail_with: Some(fail_with),
        },
        broker,
        clock,
        surface,
    };
    (rig, ticket.id)
}

impl Rig {
    fn cycle(&mut self, sync: bool) -> Result<broker_ui_impl::driver::CycleReport, UiError> {
        let now = self.clock.now_ms();
        cycle(&mut self.ui, &mut self.link, now, 0, sync)
    }

    fn physical_deny(&self) {
        self.surface.push(SurfaceEvent::Activated {
            at_ms: self.clock.now_ms(),
        });
        self.clock.advance(600);
        self.surface.push(SurfaceEvent::Button {
            id: BTN_DENY,
            input: InputSample {
                device: InputDevice::Mouse,
                injected: false,
                at_ms: self.clock.now_ms(),
            },
            occluded: false,
        });
    }

    fn status(&self, id: safety_broker_contract::ApprovalId) -> ApprovalStatus {
        self.broker.approval_status(id, &delta()).unwrap()
    }
}

#[test]
fn link_error_on_denial_keeps_the_card() {
    let (mut r, id) = rig(UiError::Link("potok zamknięty".into()));
    assert_eq!(r.cycle(true).unwrap().events, vec![UiEvent::Shown { id }]);
    r.physical_deny();
    let err = r.cycle(false).unwrap_err();
    assert!(matches!(err, UiError::Link(_)), "{err:?}");
    assert_eq!(r.status(id), ApprovalStatus::Pending, "odmowa nie dotarła");
    // Następna synchronizacja (po ponownym połączeniu) pokazuje kartę znowu.
    let rep = r.cycle(true).unwrap();
    assert!(rep.events.contains(&UiEvent::Shown { id }), "{rep:?}");
    assert_eq!(r.ui.queued(), vec![id]);
    r.physical_deny();
    assert_eq!(r.cycle(false).unwrap().resolved, Some(id));
    assert_eq!(r.status(id), ApprovalStatus::Denied);
}

#[test]
fn explicit_rejection_of_denial_ends_the_card() {
    let (mut r, id) = rig(UiError::Rejected("audyt niedostępny".into()));
    r.cycle(true).unwrap();
    r.physical_deny();
    assert_eq!(r.cycle(false).unwrap().resolved, Some(id));
    assert!(r.ui.queued().is_empty());
}
