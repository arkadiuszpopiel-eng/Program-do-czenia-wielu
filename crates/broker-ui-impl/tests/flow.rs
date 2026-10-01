//! Pełny cykl na atrapach: prośba z Brokera (prawdziwy silnik w `safety-broker-fake`) → karta
//! w oknie (`FakeSurface`) → decyzja z dowodem → Broker. Clickjacking, wstrzyknięcia, Esc,
//! „zawsze w tym zakresie”, wygaśnięcie, Windows Hello.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use broker_ui_contract::{
    BrokerUi, HelloOutcome, HelloPort, NoHello, RejectReason, UiConfig, UiEvent, UiStatus,
};
use broker_ui_impl::driver::cycle;
use broker_ui_impl::{BTN_DENY, BTN_ONCE, BTN_SCOPED, ChannelLink, NativeBrokerUi};
use compliance_contract::PathEnv;
use platform_contract::{InputDevice, InputSample, SurfaceEvent};
use platform_fake::FakeSurface;
use safety_broker_contract::contract_tests::{PROFILE, delta, host, request, test_policy};
use safety_broker_contract::{
    ApprovalId, ApprovalStatus, Broker, Capability, CommandOrigin, Decision, HelloRequirement,
};
use safety_broker_fake::FakeBroker;
use watchdog_contract::{Clock, ManualClock};

struct Rig {
    broker: Arc<FakeBroker>,
    clock: Arc<ManualClock>,
    surface: Arc<FakeSurface>,
    ui: NativeBrokerUi<FakeSurface>,
    link: ChannelLink<FakeBroker>,
    rt: tokio::runtime::Runtime,
}

struct Hello(HelloOutcome);
impl HelloPort for Hello {
    fn verify(&self, _: &str) -> HelloOutcome {
        self.0
    }
}

fn rig_with(policy: safety_broker_contract::KernelPolicy, hello: Arc<dyn HelloPort>) -> Rig {
    let clock = Arc::new(ManualClock::new(1_000_000));
    let broker = Arc::new(
        FakeBroker::with(policy, PathEnv::windows_profile(PROFILE), clock.clone()).unwrap(),
    );
    let surface = Arc::new(FakeSurface::new());
    let config = UiConfig {
        hello_enabled: true,
        ..UiConfig::default()
    };
    Rig {
        ui: NativeBrokerUi::new(surface.clone(), hello, config),
        link: ChannelLink::new(broker.clone()).unwrap(),
        rt: tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap(),
        broker,
        clock,
        surface,
    }
}

fn rig() -> Rig {
    rig_with(test_policy(), Arc::new(NoHello))
}

impl Rig {
    fn ask(&self) -> ApprovalId {
        let req = request(
            &delta(),
            Capability::NetEgress(host("x.example.org")),
            CommandOrigin::UserText,
        );
        match self.rt.block_on(self.broker.decide(req)).unwrap() {
            Decision::NeedsApproval(t) => t.id,
            other => panic!("{other:?}"),
        }
    }

    fn step(&mut self, sync: bool) -> broker_ui_impl::driver::CycleReport {
        let now = self.clock.now_ms();
        cycle(&mut self.ui, &mut self.link, now, 0, sync).unwrap()
    }

    fn input(&self, device: InputDevice, injected: bool) -> InputSample {
        InputSample {
            device,
            injected,
            at_ms: self.clock.now_ms(),
        }
    }

    fn click(&self, id: u16, injected: bool) {
        let input = self.input(InputDevice::Mouse, injected);
        self.surface.push(SurfaceEvent::Button {
            id,
            input,
            occluded: false,
        });
    }

    fn status(&self, id: ApprovalId) -> ApprovalStatus {
        self.broker.approval_status(id, &delta()).unwrap()
    }
}

#[test]
fn physical_click_after_500ms_approves_and_token_is_issued() {
    let mut r = rig();
    let id = r.ask();
    let rep = r.step(true);
    assert_eq!(rep.events, vec![UiEvent::Shown { id }]);
    let view = r.surface.current().unwrap();
    assert_eq!(view.title, "Delta prosi o zgodę: wysłanie danych do sieci");
    assert_eq!(view.initial_focus, BTN_DENY);
    assert!(view.badge.contains("Ryzyko"));
    assert_eq!(r.ui.status(), UiStatus::Pending { count: 1 });
    r.surface.push(SurfaceEvent::Activated {
        at_ms: r.clock.now_ms(),
    });
    r.step(false);
    r.clock.advance(600);
    r.click(BTN_ONCE, false);
    let rep = r.step(false);
    assert_eq!(rep.resolved, Some(id));
    assert!(matches!(
        r.status(id),
        ApprovalStatus::Approved { token: Some(_) }
    ));
    assert_eq!(r.ui.status(), UiStatus::Hidden);
    assert_eq!(r.surface.dismissals(), 1);
}

#[test]
fn clickjacking_injection_and_occlusion_are_rejected() {
    let mut r = rig();
    let id = r.ask();
    r.step(true);
    r.surface.push(SurfaceEvent::Activated {
        at_ms: r.clock.now_ms(),
    });
    r.clock.advance(100);
    r.click(BTN_ONCE, false);
    let rep = r.step(false);
    assert_eq!(rep.resolved, None);
    assert!(matches!(
        &rep.events[..],
        [UiEvent::InputRejected {
            reason: RejectReason::TooSoon { elapsed_ms: 100 },
            ..
        }]
    ));
    assert!(
        r.surface
            .current()
            .unwrap()
            .status
            .contains("kliknij ponownie")
    );
    r.clock.advance(1_000);
    r.click(BTN_ONCE, true);
    let rep = r.step(false);
    assert_eq!(rep.events[0].name(), "broker_ui.injection_rejected");
    let input = r.input(InputDevice::Mouse, false);
    r.surface.push(SurfaceEvent::Button {
        id: BTN_ONCE,
        input,
        occluded: true,
    });
    assert_eq!(r.step(false).resolved, None);
    r.surface.push(SurfaceEvent::Deactivated {
        at_ms: r.clock.now_ms(),
    });
    r.click(BTN_ONCE, false);
    assert_eq!(r.step(false).resolved, None, "okno nieaktywne");
    assert_eq!(r.status(id), ApprovalStatus::Pending);
    r.surface.push(SurfaceEvent::Activated {
        at_ms: r.clock.now_ms(),
    });
    r.clock.advance(500);
    let kb = r.input(InputDevice::Keyboard, false);
    r.surface.push(SurfaceEvent::Cancel { input: kb });
    assert_eq!(r.step(false).resolved, Some(id));
    assert_eq!(r.status(id), ApprovalStatus::Denied);
}

#[test]
fn scoped_grant_and_second_card_waits_its_own_500ms() {
    let mut r = rig();
    let first = r.ask();
    let second = r.ask();
    r.step(true);
    r.surface.push(SurfaceEvent::Activated {
        at_ms: r.clock.now_ms(),
    });
    r.clock.advance(700);
    r.click(BTN_SCOPED, false);
    assert_eq!(r.step(false).resolved, Some(first));
    r.step(true);
    assert_eq!(r.ui.queued(), vec![second]);
    r.click(BTN_ONCE, false);
    let rep = r.step(false);
    assert_eq!(
        rep.resolved, None,
        "nowa karta w aktywnym oknie — znów 500 ms"
    );
    r.clock.advance(500);
    r.click(BTN_ONCE, false);
    assert_eq!(r.step(false).resolved, Some(second));
    let again = request(
        &delta(),
        Capability::NetEgress(host("x.example.org")),
        CommandOrigin::UserText,
    );
    assert!(matches!(
        r.rt.block_on(r.broker.decide(again)).unwrap(),
        Decision::Allow(_)
    ));
}

#[test]
fn expiry_and_external_resolution_withdraw_the_card() {
    let mut r = rig();
    let id = r.ask();
    r.step(true);
    r.clock.advance(test_policy().approval_ttl_ms);
    let rep = r.step(true);
    assert!(rep.events.contains(&UiEvent::Withdrawn { id }));
    assert!(r.ui.queued().is_empty());
    assert_eq!(r.status(id), ApprovalStatus::Expired);
}

#[test]
fn hello_required_uses_hello_or_refuses() {
    let mut policy = test_policy();
    policy.hello_required_for = vec![HelloRequirement::Policy];
    let mut r = rig_with(policy.clone(), Arc::new(Hello(HelloOutcome::Verified)));
    let id =
        r.rt.block_on(r.broker.request_policy_change(
            policy.clone(),
            safety_broker_contract::ChangeOrigin::UserInterface,
        ))
        .unwrap();
    r.step(true);
    assert!(
        r.surface.current().unwrap().buttons[1]
            .label
            .contains("Windows Hello")
    );
    r.surface.push(SurfaceEvent::Activated {
        at_ms: r.clock.now_ms(),
    });
    r.clock.advance(600);
    r.click(BTN_ONCE, false);
    let rep = r.step(false);
    assert_eq!(rep.resolved, Some(id));
    assert!(
        rep.events
            .contains(&UiEvent::HelloUsed { id, verified: true })
    );

    let mut r = rig_with(policy.clone(), Arc::new(Hello(HelloOutcome::Cancelled)));
    r.rt.block_on(
        r.broker
            .request_policy_change(policy, safety_broker_contract::ChangeOrigin::UserInterface),
    )
    .unwrap();
    r.step(true);
    r.surface.push(SurfaceEvent::Activated {
        at_ms: r.clock.now_ms(),
    });
    r.clock.advance(600);
    r.click(BTN_ONCE, false);
    let rep = r.step(false);
    assert_eq!(rep.resolved, None);
    assert!(rep.events.iter().any(|e| matches!(
        e,
        UiEvent::InputRejected {
            reason: RejectReason::HelloFailed,
            ..
        }
    )));
}
