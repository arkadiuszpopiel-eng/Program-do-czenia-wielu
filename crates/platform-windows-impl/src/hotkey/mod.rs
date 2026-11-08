//! `HotkeyPort`: skróty globalne `RegisterHotKey` na dedykowanym wątku z pętlą komunikatów +
//! hook `WH_KEYBOARD_LL` zgłaszający puszczenie klawisza (PTT, PLAN §7.3). Walidacja przez
//! `Hotkey::validate()` z kontraktu (reguła AltGr, rezerwacja kill-switcha) przed dotknięciem systemu;
//! skrót zajęty przez inną aplikację → `PlatformError::HotkeyConflict`. Naciśnięcie z wejścia
//! wstrzykniętego (hook: `LLKHF_INJECTED`) jest ignorowane; kill-switch działa zawsze
//! (przegląd #2, P2-04 — `origin.rs`).

mod keys;
#[cfg_attr(not(windows), allow(dead_code))]
mod origin;
#[cfg(windows)]
mod thread;

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use platform_contract::{Hotkey, HotkeyEvent, HotkeyId, HotkeyPort, KILL_SWITCH, PlatformError};

/// Maksymalna liczba nieodebranych zdarzeń (najstarsze są odrzucane).
const QUEUE_CAPACITY: usize = 1024;

/// Kolejka zdarzeń współdzielona z wątkiem skrótów (hook dopisuje, konsument odbiera).
#[derive(Debug, Default)]
pub(crate) struct EventQueue {
    queue: Mutex<VecDeque<HotkeyEvent>>,
    ready: Condvar,
}

impl EventQueue {
    fn lock(&self) -> MutexGuard<'_, VecDeque<HotkeyEvent>> {
        self.queue.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Dopisuje zdarzenie i budzi czekających.
    pub(crate) fn push(&self, event: HotkeyEvent) {
        let mut queue = self.lock();
        if queue.len() >= QUEUE_CAPACITY {
            queue.pop_front();
        }
        queue.push_back(event);
        self.ready.notify_all();
    }

    /// Zabiera wszystkie zdarzenia (FIFO).
    pub(crate) fn drain(&self) -> Vec<HotkeyEvent> {
        self.lock().drain(..).collect()
    }

    /// Czeka na zdarzenia co najwyżej `timeout`, potem zabiera wszystkie.
    pub(crate) fn wait(&self, timeout: Duration) -> Vec<HotkeyEvent> {
        let guard = self.lock();
        let (mut guard, _) = self
            .ready
            .wait_timeout_while(guard, timeout, |q| q.is_empty())
            .unwrap_or_else(|p| p.into_inner());
        guard.drain(..).collect()
    }
}

#[derive(Debug, Default)]
struct State {
    registered: BTreeMap<u32, Hotkey>,
    next_id: u32,
    #[cfg(windows)]
    thread: Option<thread::HotkeyThread>,
}

/// Skróty globalne Windows.
#[derive(Debug, Default)]
pub struct WinHotkeys {
    state: Mutex<State>,
    events: Arc<EventQueue>,
}

impl WinHotkeys {
    /// Nowy rejestr (wątek komunikatów startuje przy pierwszej rejestracji).
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Rejestruje kill-switch `Ctrl+Shift+F12` — wyłącznie dla watchdoga/Brokera (PLAN §8.6);
    /// `register` odrzuca ten skrót jako zarezerwowany.
    pub fn register_kill_switch(&self) -> Result<HotkeyId, PlatformError> {
        self.register_unchecked(KILL_SWITCH)
    }

    /// Czeka na zdarzenia (niska latencja PTT bez aktywnego odpytywania).
    pub fn wait_events(&self, timeout: Duration) -> Vec<HotkeyEvent> {
        self.events.wait(timeout)
    }

    /// Zarejestrowane skróty.
    pub fn registered(&self) -> Vec<(HotkeyId, Hotkey)> {
        self.lock()
            .registered
            .iter()
            .map(|(id, hk)| (HotkeyId(*id), *hk))
            .collect()
    }

    fn register_unchecked(&self, hotkey: Hotkey) -> Result<HotkeyId, PlatformError> {
        let native = keys::to_native(&hotkey)?;
        let mut state = self.lock();
        if state.registered.values().any(|hk| *hk == hotkey) {
            return Err(PlatformError::HotkeyRejected(format!(
                "{hotkey} już zarejestrowany"
            )));
        }
        let id = state.next_id + 1;
        self.register_native(&mut state, id, native, &hotkey)?;
        state.next_id = id;
        state.registered.insert(id, hotkey);
        Ok(HotkeyId(id))
    }

    #[cfg(windows)]
    fn register_native(
        &self,
        state: &mut State,
        id: u32,
        native: keys::NativeHotkey,
        hotkey: &Hotkey,
    ) -> Result<(), PlatformError> {
        if state.thread.is_none() {
            state.thread = Some(thread::HotkeyThread::start(Arc::clone(&self.events))?);
        }
        match &state.thread {
            Some(worker) => worker.register(id, native, *hotkey),
            None => Err(PlatformError::Io("wątek skrótów nie wystartował".into())),
        }
    }

    #[cfg(not(windows))]
    fn register_native(
        &self,
        _state: &mut State,
        _id: u32,
        _native: keys::NativeHotkey,
        hotkey: &Hotkey,
    ) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported(format!(
            "{hotkey}: skróty globalne tylko na Windows"
        )))
    }
}

impl HotkeyPort for WinHotkeys {
    fn register(&self, hotkey: Hotkey) -> Result<HotkeyId, PlatformError> {
        hotkey.validate()?;
        self.register_unchecked(hotkey)
    }

    fn unregister(&self, id: HotkeyId) -> Result<(), PlatformError> {
        let mut state = self.lock();
        if !state.registered.contains_key(&id.0) {
            return Err(PlatformError::UnknownResource(format!("skrót {}", id.0)));
        }
        #[cfg(windows)]
        if let Some(worker) = &state.thread {
            worker.unregister(id.0)?;
        }
        state.registered.remove(&id.0);
        Ok(())
    }

    fn drain_events(&self) -> Vec<HotkeyEvent> {
        self.events.drain()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_contract::{Key, Modifiers};

    fn ctrl_alt(shift: bool, key: Key) -> Hotkey {
        Hotkey::new(
            Modifiers {
                ctrl: true,
                alt: true,
                shift,
                win: false,
            },
            key,
        )
    }

    #[test]
    fn altgr_rule_and_kill_switch_rejected_before_system_call() {
        let hk = WinHotkeys::new();
        for c in "acelnosxzACELNOSXZ".chars() {
            for shift in [false, true] {
                assert!(
                    matches!(
                        hk.register(ctrl_alt(shift, Key::Letter(c))),
                        Err(PlatformError::HotkeyRejected(_))
                    ),
                    "Ctrl+Alt{}+{c}",
                    if shift { "+Shift" } else { "" }
                );
            }
        }
        assert!(matches!(
            hk.register(KILL_SWITCH),
            Err(PlatformError::HotkeyRejected(_))
        ));
        assert!(matches!(
            hk.register(ctrl_alt(false, Key::Letter('ś'))),
            Err(PlatformError::HotkeyRejected(_))
        ));
        assert!(matches!(
            hk.unregister(HotkeyId(77)),
            Err(PlatformError::UnknownResource(_))
        ));
        if !cfg!(windows) {
            assert!(matches!(
                hk.register(ctrl_alt(false, Key::Space)),
                Err(PlatformError::Unsupported(_))
            ));
            assert!(hk.register_kill_switch().is_err());
            assert!(hk.registered().is_empty());
        }
    }

    #[test]
    fn event_queue_is_fifo_bounded_and_wakes_waiters() {
        let queue = Arc::new(EventQueue::default());
        assert!(queue.wait(Duration::from_millis(5)).is_empty());
        for i in 0..(QUEUE_CAPACITY as u32 + 3) {
            queue.push(HotkeyEvent {
                id: HotkeyId(i),
                pressed: i % 2 == 0,
            });
        }
        let all = queue.drain();
        assert_eq!(all.len(), QUEUE_CAPACITY);
        assert_eq!(all[0].id, HotkeyId(3));
        let producer = Arc::clone(&queue);
        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            producer.push(HotkeyEvent {
                id: HotkeyId(1),
                pressed: false,
            });
        });
        let got = queue.wait(Duration::from_secs(5));
        handle.join().unwrap();
        assert_eq!(got.len(), 1);
        assert!(!got[0].pressed);
        let hk = WinHotkeys::new();
        assert!(hk.drain_events().is_empty());
        assert!(hk.wait_events(Duration::from_millis(1)).is_empty());
    }
}
