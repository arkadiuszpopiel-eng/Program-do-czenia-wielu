//! Testy usługi: kontrakt na atrapie portu skrótów, okno administratora, mikrofon w schedulerze, moduł.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext, ModuleError};
use personas_contract::{Catalog, builtin_personas};
use platform_contract::{Hotkey, HotkeyPort, Key, Modifiers};
use platform_fake::{FakeHotkeys, FakeProcesses};
use scheduler_lite_contract::{Holder, Resource, SchedulerLite};
use scheduler_lite_fake::FakeScheduler;
use voice_wake_contract::contract_tests::{self, KeyDriver};
use voice_wake_contract::{Wake, WakeCfg, WakeEvent, WakeSource, event_kind};
use voice_wake_impl::{MODULE_TOML, MicArbiter, VoiceWakeModule, WakeService};

/// Klawiatura przez atrapę `HotkeyPort` (jak hook: wciśnięcie/puszczenie zarejestrowanego skrótu).
struct Keys(Arc<FakeHotkeys>);

impl Keys {
    fn id_of(&self, hk: Hotkey) -> platform_contract::HotkeyId {
        self.0
            .registered()
            .into_iter()
            .find(|(_, h)| *h == hk)
            .map(|(id, _)| id)
            .unwrap()
    }
}

impl KeyDriver for Keys {
    fn ptt(&self, pressed: bool) {
        let id = self.id_of(WakeCfg::default().ptt_key.unwrap());
        if pressed {
            self.0.press(id).unwrap()
        } else {
            self.0.release(id).unwrap()
        }
    }
    fn toggle(&self) {
        let id = self.id_of(WakeCfg::default().toggle_key.unwrap());
        self.0.press(id).unwrap();
        self.0.release(id).unwrap();
    }
}

fn service(hk: &Arc<FakeHotkeys>) -> WakeService {
    WakeService::new(
        hk.clone(),
        builtin_personas(),
        Some(Catalog::builtin().default_cast(true)),
    )
}

#[test]
fn contract_suite() {
    contract_tests::run_all(|| {
        let hk = Arc::new(FakeHotkeys::new());
        (service(&hk), Box::new(Keys(hk)) as Box<dyn KeyDriver>)
    });
}

#[test]
fn registers_unregisters_and_detects_elevated_window() {
    let hk = Arc::new(FakeHotkeys::new());
    let procs = Arc::new(FakeProcesses::default());
    let mut w = service(&hk).with_processes(procs.clone());
    w.configure(WakeCfg::default()).unwrap();
    assert_eq!(hk.registered().len(), 2);
    w.configure(WakeCfg {
        toggle_key: None,
        ..WakeCfg::default()
    })
    .unwrap();
    assert_eq!(hk.registered().len(), 1, "stare skróty wyrejestrowane");
    procs.set_foreground_elevated(true);
    assert_eq!(w.pump(), vec![WakeEvent::BlockedElevatedForeground]);
    assert!(w.pump().is_empty());
    let bad = WakeCfg {
        ptt_key: Some(Hotkey::new(Modifiers::default(), Key::Letter('A'))),
        ..WakeCfg::default()
    };
    assert!(w.configure(bad).is_err());
    assert_eq!(
        hk.registered().len(),
        1,
        "niepoprawna konfiguracja nie psuje poprzedniej"
    );
    w.set_cast(None);
    drop(w);
    assert!(hk.drain_events().is_empty());
}

#[tokio::test]
async fn microphone_is_an_exclusive_lease_while_listening() {
    let hk = Arc::new(FakeHotkeys::new());
    let sched = Arc::new(FakeScheduler::new());
    let mut w = service(&hk);
    w.configure(WakeCfg::default()).unwrap();
    let keys = Keys(hk.clone());
    let mut mic = MicArbiter::new(sched.clone(), Duration::ZERO);
    keys.ptt(true);
    mic.apply(&w.pump()).await.unwrap();
    assert!(mic.held());
    assert_eq!(sched.holder(&Resource::Mic).unwrap().holder, Holder::User);
    keys.ptt(false);
    let ev = w.pump();
    assert!(ev.contains(&WakeEvent::ListenStop {
        source: WakeSource::Ptt
    }));
    mic.apply(&ev).await.unwrap();
    assert!(!mic.held());
    assert!(sched.holder(&Resource::Mic).is_none());
    assert!(format!("{mic:?}").contains("held"));
}

#[tokio::test]
async fn module_publishes_events() {
    let mut m = VoiceWakeModule::new().unwrap();
    assert!(MODULE_TOML.contains("voice-wake-contract@1"));
    assert_eq!(m.health(), HealthStatus::NotStarted);
    let ev = [WakeEvent::Dnd { on: true }];
    assert_eq!(m.publish(&ev).await, 0);
    let bus = FakeBus::default();
    m.start(ModuleContext::new(
        m.manifest().id.clone(),
        Arc::new(bus.clone()),
    ))
    .await
    .unwrap();
    assert_eq!(m.publish(&ev).await, 1);
    assert_eq!(bus.recorded_of_kind(&event_kind("voice.wake.dnd")).len(), 1);
    assert_eq!(m.health(), HealthStatus::Healthy);
    let ctx = ModuleContext::new(m.manifest().id.clone(), Arc::new(bus));
    assert_eq!(m.start(ctx).await, Err(ModuleError::AlreadyStarted));
    m.stop().await.unwrap();
    assert_eq!(m.stop().await, Err(ModuleError::NotStarted));
}
