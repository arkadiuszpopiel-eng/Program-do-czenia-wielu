//! Fake skrótów globalnych ze skryptowaniem zdarzeń.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use platform_contract::{
    Hotkey, HotkeyEvent, HotkeyId, HotkeyPort, HotkeyPressOrigin, PlatformError,
};

#[derive(Debug, Default)]
struct State {
    registered: BTreeMap<u32, Hotkey>,
    queue: Vec<HotkeyEvent>,
    next_id: u32,
}

/// Rejestr skrótów w pamięci; test symuluje naciśnięcia przez `press`/`release`.
#[derive(Debug, Default)]
pub struct FakeHotkeys {
    state: Mutex<State>,
}

impl FakeHotkeys {
    /// Pusty rejestr.
    pub fn new() -> Self {
        Self::default()
    }

    /// Zarejestrowane skróty.
    pub fn registered(&self) -> Vec<(HotkeyId, Hotkey)> {
        self.lock()
            .registered
            .iter()
            .map(|(id, hk)| (HotkeyId(*id), *hk))
            .collect()
    }

    /// Symuluje naciśnięcie zarejestrowanego skrótu.
    pub fn press(&self, id: HotkeyId) -> Result<(), PlatformError> {
        self.push(id, true)
    }

    /// Symuluje puszczenie zarejestrowanego skrótu (PTT).
    pub fn release(&self, id: HotkeyId) -> Result<(), PlatformError> {
        self.push(id, false)
    }

    /// Symuluje naciśnięcie kombinacji; zdarzenie powstaje tylko, jeśli jest zarejestrowana.
    pub fn press_combo(&self, hotkey: Hotkey) -> bool {
        let mut st = self.lock();
        let found = st
            .registered
            .iter()
            .find(|(_, hk)| **hk == hotkey)
            .map(|(id, _)| *id);
        match found {
            Some(id) => {
                st.queue.push(HotkeyEvent {
                    id: HotkeyId(id),
                    pressed: true,
                });
                true
            }
            None => false,
        }
    }

    /// Symuluje kombinację wysłaną wejściem wstrzykniętym (`SendInput`) — jak Windows (przegląd
    /// #2, P2-04): skróty Alfy jej nie przyjmują; zdarzenie powstaje tylko dla kill-switcha.
    pub fn press_injected(&self, hotkey: Hotkey) -> bool {
        HotkeyPressOrigin::Injected.admits(&hotkey, false) && self.press_combo(hotkey)
    }

    fn push(&self, id: HotkeyId, pressed: bool) -> Result<(), PlatformError> {
        let mut st = self.lock();
        if !st.registered.contains_key(&id.0) {
            return Err(PlatformError::UnknownResource(format!("skrót {}", id.0)));
        }
        st.queue.push(HotkeyEvent { id, pressed });
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl HotkeyPort for FakeHotkeys {
    fn register(&self, hotkey: Hotkey) -> Result<HotkeyId, PlatformError> {
        hotkey.validate()?;
        let mut st = self.lock();
        if st.registered.values().any(|hk| *hk == hotkey) {
            return Err(PlatformError::HotkeyRejected(format!(
                "{hotkey} już zarejestrowany"
            )));
        }
        st.next_id += 1;
        let id = st.next_id;
        st.registered.insert(id, hotkey);
        Ok(HotkeyId(id))
    }

    fn unregister(&self, id: HotkeyId) -> Result<(), PlatformError> {
        self.lock()
            .registered
            .remove(&id.0)
            .map(|_| ())
            .ok_or_else(|| PlatformError::UnknownResource(format!("skrót {}", id.0)))
    }

    fn drain_events(&self) -> Vec<HotkeyEvent> {
        std::mem::take(&mut self.lock().queue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_contract::{KILL_SWITCH, Key, Modifiers};

    fn ctrl_alt(key: Key) -> Hotkey {
        Hotkey::new(
            Modifiers {
                ctrl: true,
                alt: true,
                shift: false,
                win: false,
            },
            key,
        )
    }

    #[test]
    fn register_validates_and_queues_events() {
        let hk = FakeHotkeys::new();
        assert!(hk.register(ctrl_alt(Key::Letter('S'))).is_err());
        assert!(hk.register(KILL_SWITCH).is_err());
        let id = hk.register(ctrl_alt(Key::Space)).unwrap();
        assert!(hk.register(ctrl_alt(Key::Space)).is_err());
        hk.press(id).unwrap();
        hk.release(id).unwrap();
        assert!(hk.press_combo(ctrl_alt(Key::Space)));
        assert!(!hk.press_combo(ctrl_alt(Key::Letter('B'))));
        assert!(
            !hk.press_injected(ctrl_alt(Key::Space)),
            "wstrzyknięte — ignorowane"
        );
        let events = hk.drain_events();
        assert_eq!(events.len(), 3);
        assert!(events[0].pressed && !events[1].pressed);
        assert!(hk.drain_events().is_empty());
        hk.unregister(id).unwrap();
        assert!(hk.press(id).is_err());
        assert!(hk.unregister(id).is_err());
    }
}
