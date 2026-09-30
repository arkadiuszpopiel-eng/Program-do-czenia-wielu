//! Test kontraktowy na atrapie + sterowanie zdarzeniami.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use device_profile_contract::{
    BASELINE_LIMITS, DeviceEvent, DeviceProfile, HwClass, MachineOverlay, PowerState, VoiceChoice,
    VoiceProfile, contract_tests, fixtures,
};
use device_profile_fake::FakeDeviceProfile;

#[test]
fn contract_suite_on_all_fixtures() {
    contract_tests::run_all(FakeDeviceProfile::baseline);
    contract_tests::run_all(FakeDeviceProfile::desktop);
    contract_tests::run_all(FakeDeviceProfile::laptop);
}

#[test]
fn scripted_power_fullscreen_hot_plug_and_override() {
    let fake = FakeDeviceProfile::laptop();
    assert_eq!(fake.recommend().voice_profile, VoiceProfile::D);
    fake.set_power(PowerState::Battery { percent: Some(55) });
    fake.set_power(PowerState::Battery { percent: Some(55) });
    assert!(fake.recommend().power_saving);
    fake.set_fullscreen(true);
    assert!(fake.fullscreen_active());
    let id = fake.current().machine_id;
    fake.hot_plug(fixtures::desktop());
    assert!(fake.refresh().unwrap());
    assert!(!fake.refresh().unwrap());
    assert_eq!(
        fake.current().machine_id,
        id,
        "ten sam MachineId po wymianie sprzętu"
    );
    fake.set_overlay(MachineOverlay {
        voice_profile: VoiceChoice::C,
        ..MachineOverlay::default()
    });
    assert_eq!(fake.recommend().voice_profile, VoiceProfile::C);
    let events = fake.drain_events();
    assert_eq!(events.len(), 4);
    assert!(matches!(events[0], DeviceEvent::PowerChanged { .. }));
    assert_eq!(events[1], DeviceEvent::FullscreenChanged { active: true });
    assert!(matches!(
        events[2],
        DeviceEvent::Changed {
            class: HwClass::StandardAmd,
            ..
        }
    ));
    assert!(matches!(events[3], DeviceEvent::Override { .. }));
    assert!(fake.drain_events().is_empty());
    fake.emulate(Some(BASELINE_LIMITS)).unwrap();
    assert_eq!(fake.recommend().class, HwClass::Baseline);
    fake.hot_plug(fake.current());
    assert!(fake.refresh().is_ok());
}
