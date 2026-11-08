//! Atrapa `voice-wake`: wspólny automat z kontraktu + wejścia ze skryptu na wirtualnym zegarze
//! (PTT, przełącznik, transkrypty, DND) i bezpośrednie „klawisze” do testów; v1: deterministyczne
//! modele słów wywoławczych bez ONNX ([`ToneScorer`], [`ScriptedScorer`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod scorer;

pub use scorer::{BUILTIN_TONES, ScriptedScorer, ToneScorer, WINDOW};

use std::sync::{Arc, Mutex};
use std::time::Duration;

use personas_contract::{Catalog, PersonaId, builtin_personas};
use platform_contract::HotkeyId;
use voice_wake_contract::contract_tests::KeyDriver;
use voice_wake_contract::{
    KwsParams, MicState, Wake, WakeCfg, WakeError, WakeEvent, WakeInput, WakeMachine,
};

/// Identyfikator PTT w atrapie.
pub const PTT_ID: HotkeyId = HotkeyId(1);
/// Identyfikator przełącznika w atrapie.
pub const TOGGLE_ID: HotkeyId = HotkeyId(2);

/// Atrapa aktywacji.
#[derive(Debug)]
pub struct FakeWake {
    machine: WakeMachine,
    now: Duration,
    script: Vec<(Duration, WakeInput)>,
    pending: Arc<Mutex<Vec<WakeInput>>>,
    configured: Option<WakeCfg>,
}

impl Default for FakeWake {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeWake {
    /// Wbudowane persony, domyślna obsada głosowa.
    pub fn new() -> Self {
        let cast = Catalog::builtin().default_cast(true);
        let mut machine = WakeMachine::new(builtin_personas(), Some(cast));
        machine.set_keys(Some(PTT_ID), Some(TOGGLE_ID));
        Self {
            machine,
            now: Duration::ZERO,
            script: Vec::new(),
            pending: Arc::default(),
            configured: None,
        }
    }

    /// Planuje wejście na chwilę `at` (czas wirtualny).
    pub fn schedule(&mut self, at: Duration, input: WakeInput) {
        self.script.push((at, input));
        self.script.sort_by_key(|(t, _)| *t);
    }

    /// Przesuwa zegar i zwraca zdarzenia wejść, których czas minął.
    pub fn advance(&mut self, d: Duration) -> Vec<WakeEvent> {
        self.now += d;
        let now = self.now;
        let due: Vec<WakeInput> = {
            let split = self.script.partition_point(|(t, _)| *t <= now);
            self.script.drain(..split).map(|(_, i)| i).collect()
        };
        due.into_iter()
            .flat_map(|i| self.machine.handle(i))
            .collect()
    }

    /// Uchwyt „klawiatury” (kolejka zdarzeń skrótów odbieranych przez `pump`).
    pub fn keys(&self) -> FakeKeys {
        FakeKeys(Arc::clone(&self.pending))
    }

    /// Ostatnia konfiguracja.
    pub fn configured(&self) -> Option<&WakeCfg> {
        self.configured.as_ref()
    }
}

/// „Klawiatura” atrapy: PTT i przełącznik jak z hooka `WH_KEYBOARD_LL`.
#[derive(Debug, Clone)]
pub struct FakeKeys(Arc<Mutex<Vec<WakeInput>>>);

impl FakeKeys {
    fn push(&self, input: WakeInput) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).push(input);
    }
}

impl KeyDriver for FakeKeys {
    fn ptt(&self, pressed: bool) {
        self.push(WakeInput::Key {
            id: PTT_ID,
            pressed,
        });
    }

    fn toggle(&self) {
        self.push(WakeInput::Key {
            id: TOGGLE_ID,
            pressed: true,
        });
        self.push(WakeInput::Key {
            id: TOGGLE_ID,
            pressed: false,
        });
    }
}

impl Wake for FakeWake {
    fn configure(&mut self, cfg: WakeCfg) -> Result<(), WakeError> {
        cfg.validate()?;
        self.machine.set_keys(
            cfg.ptt_key.map(|_| PTT_ID),
            cfg.toggle_key.map(|_| TOGGLE_ID),
        );
        self.machine.set_name_addressing(cfg.name_addressing);
        self.machine.set_wake_words(
            cfg.wake_words.is_some(),
            KwsParams::default().listen_timeout_ms,
        );
        self.configured = Some(cfg);
        Ok(())
    }

    fn pump(&mut self) -> Vec<WakeEvent> {
        let inputs: Vec<WakeInput> = self
            .pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .drain(..)
            .collect();
        inputs
            .into_iter()
            .flat_map(|i| self.machine.handle(i))
            .collect()
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
