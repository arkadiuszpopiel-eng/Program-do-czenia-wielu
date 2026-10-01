//! Implementacja `voice-wake` v0 (docs/modules/voice-wake/SPEC.md): skróty przez `HotkeyPort`
//! (`platform-windows-impl`: `RegisterHotKey` + hook `WH_KEYBOARD_LL` zgłaszający puszczenie PTT),
//! wykrywanie okna administratora na pierwszym planie (`ProcessPort::foreground_is_elevated`),
//! mikrofon jako zasób wyłączny w `scheduler-lite` ([`MicArbiter`]), zdarzenia na magistralę.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod mic;

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventBus;
use core_registry_contract::{
    HealthStatus, ManifestError, Module, ModuleContext, ModuleError, ModuleManifest,
};
pub use mic::MicArbiter;
use personas_contract::{Cast, Persona, PersonaId};
use platform_contract::{HotkeyId, HotkeyPort, ProcessPort};
use voice_wake_contract::{MicState, Wake, WakeCfg, WakeError, WakeEvent, WakeInput, WakeMachine};

/// Treść `module.toml`.
pub const MODULE_TOML: &str = include_str!("../module.toml");

/// Usługa aktywacji na portach platformy.
pub struct WakeService {
    hotkeys: Arc<dyn HotkeyPort>,
    processes: Option<Arc<dyn ProcessPort>>,
    machine: WakeMachine,
    registered: Vec<HotkeyId>,
}

impl std::fmt::Debug for WakeService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WakeService")
            .field("registered", &self.registered)
            .finish_non_exhaustive()
    }
}

impl WakeService {
    /// Usługa z portem skrótów, personami i obsadą (Dyrygentka).
    pub fn new(hotkeys: Arc<dyn HotkeyPort>, personas: Vec<Persona>, cast: Option<Cast>) -> Self {
        Self {
            hotkeys,
            processes: None,
            machine: WakeMachine::new(personas, cast),
            registered: Vec::new(),
        }
    }

    /// Sprawdzanie okna podniesionego przy każdym `pump`.
    #[must_use]
    pub fn with_processes(mut self, processes: Arc<dyn ProcessPort>) -> Self {
        self.processes = Some(processes);
        self
    }

    /// Zmiana obsady (np. przełączenie sesji).
    pub fn set_cast(&mut self, cast: Option<Cast>) {
        self.machine.set_cast(cast);
    }

    fn register_keys(
        &mut self,
        cfg: &WakeCfg,
    ) -> Result<(Option<HotkeyId>, Option<HotkeyId>), WakeError> {
        let mut ids = [None, None];
        for (slot, key) in ids.iter_mut().zip([cfg.ptt_key, cfg.toggle_key]) {
            if let Some(hk) = key {
                let id = self
                    .hotkeys
                    .register(hk)
                    .map_err(|e| WakeError::Hotkey(e.to_string()))?;
                self.registered.push(id);
                *slot = Some(id);
            }
        }
        Ok((ids[0], ids[1]))
    }

    fn unregister_all(&mut self) {
        for id in self.registered.drain(..) {
            let _ = self.hotkeys.unregister(id);
        }
    }
}

impl Drop for WakeService {
    fn drop(&mut self) {
        self.unregister_all();
    }
}

impl Wake for WakeService {
    fn configure(&mut self, cfg: WakeCfg) -> Result<(), WakeError> {
        cfg.validate()?;
        self.unregister_all();
        let result = self.register_keys(&cfg);
        match result {
            Ok((ptt, toggle)) => {
                self.machine.set_keys(ptt, toggle);
                self.machine.set_name_addressing(cfg.name_addressing);
                Ok(())
            }
            Err(e) => {
                self.unregister_all();
                self.machine.set_keys(None, None);
                Err(e)
            }
        }
    }

    fn pump(&mut self) -> Vec<WakeEvent> {
        let mut out = Vec::new();
        if let Some(p) = &self.processes {
            let elevated = p.foreground_is_elevated();
            out.extend(
                self.machine
                    .handle(WakeInput::ElevatedForeground { elevated }),
            );
        }
        for e in self.hotkeys.drain_events() {
            out.extend(self.machine.handle(WakeInput::Key {
                id: e.id,
                pressed: e.pressed,
            }));
        }
        out
    }

    fn handle(&mut self, input: WakeInput) -> Vec<WakeEvent> {
        self.machine.handle(input)
    }

    fn addressed(&self, text: &str) -> Option<PersonaId> {
        self.machine.addressee(text)
    }

    fn mic_state(&self) -> MicState {
        self.machine.mic_state()
    }

    fn is_listening(&self) -> bool {
        self.machine.listening().is_some()
    }
}

/// Moduł `voice-wake`: publikacja zdarzeń.
pub struct VoiceWakeModule {
    manifest: ModuleManifest,
    bus: Option<Arc<dyn EventBus>>,
}

impl VoiceWakeModule {
    /// Moduł z manifestem.
    pub fn new() -> Result<Self, ManifestError> {
        Ok(Self {
            manifest: ModuleManifest::parse_toml(MODULE_TOML)?,
            bus: None,
        })
    }

    /// Publikuje zdarzenia; zwraca liczbę opublikowanych.
    pub async fn publish(&self, events: &[WakeEvent]) -> usize {
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
}

#[async_trait]
impl Module for VoiceWakeModule {
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
        if self.bus.is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::NotStarted
        }
    }
}
