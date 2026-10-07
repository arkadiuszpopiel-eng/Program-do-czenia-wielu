//! Implementacja `voice-audio` (docs/modules/voice-audio/SPEC.md, ADR 0011): WASAPI przez crate
//! `wasapi` (tryb współdzielony, zdarzeniowy; pozycja i czas z `IAudioClock`) na Windows,
//! [`UnsupportedAudio`] na innych systemach. Logika RT (mikser, kolejki SPSC) pochodzi
//! z `voice-audio-contract` — ta sama co w atrapie. [`VoiceAudioModule`] publikuje zdarzenia
//! urządzeń na magistralę (poza wątkiem RT).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod convert;
mod unsupported;
#[cfg(windows)]
mod wasapi_io;

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
pub use unsupported::UnsupportedAudio;
use voice_audio_contract::{AudioEvent, AudioIo, DeviceEvent};
#[cfg(windows)]
pub use wasapi_io::WasapiAudio;

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Audio systemu: WASAPI na Windows, w przeciwnym razie `Unsupported`.
pub fn system_audio() -> Arc<dyn AudioIo> {
    #[cfg(windows)]
    {
        Arc::new(WasapiAudio::new())
    }
    #[cfg(not(windows))]
    {
        Arc::new(UnsupportedAudio)
    }
}

/// Moduł `voice-audio`: dostęp do [`AudioIo`] i publikacja zdarzeń urządzeń.
pub struct VoiceAudioModule {
    manifest: ModuleManifest,
    io: Arc<dyn AudioIo>,
    bus: Option<Arc<dyn EventBus>>,
}

impl VoiceAudioModule {
    /// Moduł na podanym `AudioIo` (produkcja: [`system_audio`]; testy: atrapa).
    pub fn new(io: Arc<dyn AudioIo>) -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            io,
            bus: None,
        })
    }

    /// Wejście/wyjście audio.
    pub fn io(&self) -> Arc<dyn AudioIo> {
        Arc::clone(&self.io)
    }

    /// Publikuje zdarzenia (np. z `OutputStream::poll_events`); zwraca liczbę opublikowanych.
    pub async fn publish(&self, events: &[AudioEvent]) -> usize {
        let Some(bus) = &self.bus else {
            return 0;
        };
        let mut n = 0;
        for e in events {
            if bus.publish(e.to_bus_event()).await.is_ok() {
                n += 1;
            }
        }
        n
    }

    /// Odbiera zmiany urządzeń i publikuje `voice.audio.device.changed` (+ ostrzeżenie Bluetooth).
    pub async fn pump_device_events(&self) -> Vec<DeviceEvent> {
        let changes = self.io.poll_device_events();
        let mut events = Vec::with_capacity(changes.len());
        for change in &changes {
            if let DeviceEvent::Added { device } = change
                && device.bluetooth
            {
                events.push(AudioEvent::BluetoothWarning {
                    device: device.id.clone(),
                });
            }
            events.push(AudioEvent::DeviceChanged {
                change: change.clone(),
            });
        }
        self.publish(&events).await;
        changes
    }
}

#[async_trait]
impl Module for VoiceAudioModule {
    fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError> {
        if self.bus.is_some() {
            return Err(ModuleError::AlreadyStarted);
        }
        self.bus = Some(ctx.bus);
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), ModuleError> {
        self.bus.take().map(|_| ()).ok_or(ModuleError::NotStarted)
    }

    fn health(&self) -> HealthStatus {
        if self.bus.is_none() {
            return HealthStatus::NotStarted;
        }
        match self.io.devices() {
            Ok(_) => HealthStatus::Healthy,
            Err(e) => HealthStatus::Degraded(e.to_string()),
        }
    }
}
