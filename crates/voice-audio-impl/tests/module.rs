//! Testy modułu: manifest, cykl życia, publikacja zdarzeń urządzeń, brak WASAPI poza Windows.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Lifecycle, Module, ModuleContext, ModuleError};
use voice_audio_contract::{AudioEvent, DeviceKind, Lane, event_kind};
use voice_audio_fake::FakeAudio;
use voice_audio_impl::{MODULE_TOML, VoiceAudioModule, system_audio};

#[test]
fn manifest_is_valid() {
    let m = VoiceAudioModule::new(Arc::new(FakeAudio::new())).unwrap();
    let man = m.manifest();
    assert_eq!(man.id.as_str(), "voice-audio");
    assert_eq!(man.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(man.lifecycle, Lifecycle::Lazy);
    assert!(MODULE_TOML.contains("voice-audio-contract@1"));
}

#[tokio::test]
async fn lifecycle_and_device_events_on_bus() {
    let fake = FakeAudio::new();
    let mut m = VoiceAudioModule::new(Arc::new(fake.clone())).unwrap();
    assert_eq!(m.health(), HealthStatus::NotStarted);
    assert_eq!(
        m.publish(&[AudioEvent::Underrun { lane: Lane::Voice }])
            .await,
        0
    );
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(m.manifest().id.clone(), Arc::new(bus.clone()));
    m.start(ctx.clone()).await.unwrap();
    assert_eq!(m.start(ctx).await, Err(ModuleError::AlreadyStarted));
    assert_eq!(m.health(), HealthStatus::Healthy);
    fake.plug(
        "bt-1",
        "Słuchawki (Bluetooth Hands-Free)",
        DeviceKind::Input,
    );
    fake.unplug("bt-1");
    let changes = m.pump_device_events().await;
    assert_eq!(changes.len(), 2);
    assert_eq!(
        bus.recorded_of_kind(&event_kind("voice.audio.device.changed"))
            .len(),
        2
    );
    assert_eq!(
        bus.recorded_of_kind(&event_kind("voice.audio.bluetooth_warning"))
            .len(),
        1
    );
    assert!(m.io().devices().unwrap().len() >= 2);
    m.stop().await.unwrap();
    assert_eq!(m.stop().await, Err(ModuleError::NotStarted));
}

#[cfg(not(windows))]
#[tokio::test]
async fn non_windows_reports_unsupported() {
    use voice_audio_contract::{AudioError, StreamConfig};
    let io = system_audio();
    assert!(matches!(io.devices(), Err(AudioError::Unsupported(_))));
    assert!(io.poll_device_events().is_empty());
    assert!(io.open_input(None, &StreamConfig::input_default()).is_err());
    assert!(
        io.open_output(None, &StreamConfig::output_default())
            .is_err()
    );
    assert!(
        io.open_loopback(None, &StreamConfig::input_default())
            .is_err()
    );
    let mut m = VoiceAudioModule::new(io).unwrap();
    let ctx = ModuleContext::new(m.manifest().id.clone(), Arc::new(FakeBus::default()));
    m.start(ctx).await.unwrap();
    assert!(matches!(m.health(), HealthStatus::Degraded(_)));
}

/// Kontrakt na prawdziwych urządzeniach (self-hosted runner z kartą dźwiękową / wirtualnym kablem).
#[cfg(windows)]
#[test]
#[ignore = "wymaga urządzeń audio (self-hosted runner)"]
fn contract_suite_on_hardware() {
    voice_audio_contract::contract_tests::run_all(|| {
        (
            Box::new(voice_audio_impl::WasapiAudio::new())
                as Box<dyn voice_audio_contract::AudioIo>,
            Box::new(|d: std::time::Duration| std::thread::sleep(d))
                as Box<dyn FnMut(std::time::Duration)>,
        )
    });
    let _ = system_audio();
}
