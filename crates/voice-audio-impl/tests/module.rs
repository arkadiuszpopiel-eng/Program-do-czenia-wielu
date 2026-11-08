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

/// Diagnostyka toru akustycznego (docs/plan/24 §2.4): nagrywa to, co słyszy domyślny mikrofon,
/// do pliku WAV — `ALFA_CAPTURE_WAV=<plik>`, `ALFA_CAPTURE_SECS` (domyślnie 8). Z `ALFA_CAPTURE_PLAY=<wav>`
/// po sekundzie odtwarza nagranie z głośnika (dźwięk — tylko w dzień). Wypisuje poziom sygnału.
/// Surowe nagranie można potem podać rozpoznawaniu (`ALFA_QWEN_WAV`) i porównać z wynikiem potoku.
#[cfg(windows)]
#[test]
#[ignore = "wymaga mikrofonu; z ALFA_CAPTURE_PLAY odtwarza dźwięk"]
fn capture_microphone_to_wav() {
    use std::time::{Duration, Instant};
    use voice_audio_contract::wav::{WavEncoding, encode_wav};
    use voice_audio_contract::{AudioIo, StreamConfig};

    let Some(target) = std::env::var_os("ALFA_CAPTURE_WAV") else {
        eprintln!("ALFA_CAPTURE_WAV nie ustawione — pomijam");
        return;
    };
    let secs = std::env::var("ALFA_CAPTURE_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(8);
    let io = voice_audio_impl::WasapiAudio::new();
    let mut input = io
        .open_input(None, &StreamConfig::input_default())
        .unwrap_or_else(|e| panic!("mikrofon: {e}"));
    let format = input.format();
    let started = Instant::now();
    let mut player = None;
    let mut samples: Vec<f32> = Vec::new();
    while started.elapsed() < Duration::from_secs(secs) {
        if player.is_none()
            && started.elapsed() > Duration::from_secs(1)
            && let Some(wav) = std::env::var_os("ALFA_CAPTURE_PLAY")
        {
            let script = format!(
                "(New-Object System.Media.SoundPlayer '{}').PlaySync()",
                wav.to_string_lossy().replace('\'', "''")
            );
            player = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &script])
                .spawn()
                .ok();
        }
        match input.read() {
            Some(frame) => samples.extend(
                frame
                    .pcm
                    .iter()
                    .step_by(usize::from(format.channels.max(1))),
            ),
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    if let Some(mut child) = player {
        let _ = child.wait();
    }
    assert!(!samples.is_empty(), "mikrofon nie oddał żadnej ramki");
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    let mono = voice_audio_contract::AudioFormat {
        channels: 1,
        ..format
    };
    std::fs::write(&target, encode_wav(&samples, mono, WavEncoding::Pcm16))
        .unwrap_or_else(|e| panic!("zapis: {e}"));
    eprintln!(
        "POMIAR mikrofon: {} próbek, {} Hz, szczyt {peak:.3}, RMS {rms:.4}, zgubione bloki {} → {}",
        samples.len(),
        format.sample_rate,
        input.overruns(),
        std::path::Path::new(&target).display()
    );
}
