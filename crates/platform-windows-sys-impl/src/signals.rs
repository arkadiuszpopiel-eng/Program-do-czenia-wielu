//! `WinSignals`: zapytania wprost (porty `IdlePort`, `PowerPort`, `FullscreenPort`, `SessionPort`)
//! i monitor w tle (`SystemSignalsPort`) — próbka wszystkich portów karmi `SignalMonitor` z
//! kontraktu; zdarzenia w ograniczonej kolejce, oczekujący budzeni zmienną warunkową.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use platform_contract::{
    FullscreenPort, FullscreenProbe, IdlePort, MAX_QUEUED_SIGNALS, PlatformError, PowerPort,
    PowerSnapshot, SessionPort, SessionState, SignalConfig, SignalEvent, SignalMonitor,
    SignalSample, SystemSignals, SystemSignalsPort,
};

use crate::{monitor, sys};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

struct State {
    monitor: SignalMonitor,
    queue: VecDeque<SignalEvent>,
    dropped: usize,
}

/// Stan współdzielony z wątkiem monitora.
pub(crate) struct Shared {
    state: Mutex<State>,
    cv: Condvar,
    start: Instant,
    own_pid: u32,
}

impl Shared {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// Próbka wszystkich portów → monitor → kolejka.
    pub(crate) fn sample(&self) {
        let sample = SignalSample {
            idle_ms: sys::idle_ms().ok(),
            power: sys::power().ok(),
            fullscreen: sys::fullscreen(self.own_pid).ok(),
            session: sys::session().ok(),
        };
        let now = self.now_ms();
        let mut st = lock(&self.state);
        let events = st.monitor.observe(now, sample);
        if events.is_empty() {
            return;
        }
        for ev in events {
            if st.queue.len() >= MAX_QUEUED_SIGNALS {
                st.queue.pop_front();
                st.dropped += 1;
            }
            st.queue.push_back(ev);
        }
        drop(st);
        self.cv.notify_all();
    }
}

/// Sygnały systemowe Windows.
pub struct WinSignals {
    cfg: SignalConfig,
    shared: Arc<Shared>,
    running: Mutex<Option<monitor::Monitor>>,
}

impl std::fmt::Debug for WinSignals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WinSignals")
            .field("cfg", &self.cfg)
            .finish_non_exhaustive()
    }
}

impl Default for WinSignals {
    fn default() -> Self {
        Self::new(SignalConfig::default())
    }
}

impl WinSignals {
    /// Monitor (bez wątku — uruchamia go [`WinSignals::start`]).
    pub fn new(cfg: SignalConfig) -> Self {
        Self {
            cfg,
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    monitor: SignalMonitor::new(cfg),
                    queue: VecDeque::new(),
                    dropped: 0,
                }),
                cv: Condvar::new(),
                start: Instant::now(),
                own_pid: std::process::id(),
            }),
            running: Mutex::new(None),
        }
    }

    /// Uruchamia wątek monitora (idempotentne). Poza Windows: `Unsupported`.
    pub fn start(&self) -> Result<(), PlatformError> {
        let mut running = lock(&self.running);
        if running.is_none() {
            *running = Some(monitor::Monitor::start(
                Arc::clone(&self.shared),
                self.cfg.poll_interval(),
            )?);
        }
        Ok(())
    }

    /// Zatrzymuje wątek monitora (stan i kolejka zostają).
    pub fn stop(&self) {
        lock(&self.running).take();
    }

    /// Czy wątek monitora działa.
    pub fn is_running(&self) -> bool {
        lock(&self.running).is_some()
    }

    /// Próbka teraz, na wątku wywołującego (bez monitora; testy i odświeżenie na żądanie).
    pub fn sample_now(&self) {
        self.shared.sample();
    }

    /// Ile zdarzeń odrzucono przy przepełnionej kolejce.
    pub fn dropped(&self) -> usize {
        lock(&self.shared.state).dropped
    }
}

impl IdlePort for WinSignals {
    fn idle_ms(&self) -> Result<u64, PlatformError> {
        sys::idle_ms()
    }
}

impl PowerPort for WinSignals {
    fn power(&self) -> Result<PowerSnapshot, PlatformError> {
        sys::power()
    }
}

impl FullscreenPort for WinSignals {
    fn probe(&self) -> Result<FullscreenProbe, PlatformError> {
        sys::fullscreen(self.shared.own_pid)
    }
}

impl SessionPort for WinSignals {
    fn session(&self) -> Result<SessionState, PlatformError> {
        sys::session()
    }
}

impl SystemSignalsPort for WinSignals {
    fn snapshot(&self) -> SystemSignals {
        lock(&self.shared.state).monitor.snapshot()
    }

    fn drain_events(&self) -> Vec<SignalEvent> {
        lock(&self.shared.state).queue.drain(..).collect()
    }

    fn wait_events(&self, timeout: Duration) -> Vec<SignalEvent> {
        let st = lock(&self.shared.state);
        let (mut st, _) = self
            .shared
            .cv
            .wait_timeout_while(st, timeout, |s| s.queue.is_empty())
            .unwrap_or_else(|p| p.into_inner());
        st.queue.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_ports<T: SystemSignalsPort + IdlePort + PowerPort + FullscreenPort + SessionPort>(
        _: &T,
    ) {
    }

    #[test]
    fn ports_and_portable_behaviour() {
        let s = WinSignals::default();
        assert_ports(&s);
        assert!(!s.is_running());
        assert_eq!(s.snapshot(), SystemSignals::default());
        if !cfg!(windows) {
            assert!(s.start().is_err());
            assert!(!s.is_running());
            assert!(s.idle_ms().is_err() && s.power().is_err());
            assert!(s.probe().is_err() && s.session().is_err());
            s.sample_now();
            assert!(s.drain_events().is_empty());
            assert!(s.wait_events(Duration::from_millis(5)).is_empty());
            assert_eq!(s.dropped(), 0);
        }
        s.stop();
    }
}
