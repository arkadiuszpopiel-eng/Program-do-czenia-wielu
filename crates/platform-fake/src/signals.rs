//! Atrapa sygnałów systemowych z wirtualnym zegarem: test ustawia ostatnie wejście, zasilanie,
//! pełny ekran i sesję; `advance` przesuwa zegar i próbkuje co `poll_ms` jak monitor Windows
//! (ten sam `SignalMonitor`), `notify` = natychmiastowa próbka (powiadomienie systemu).
//! Wartość `None` portu = odczyt nieudany.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use platform_contract::{
    FullscreenPort, FullscreenProbe, IdlePort, MAX_QUEUED_SIGNALS, PlatformError, PowerPort,
    PowerSnapshot, SessionPort, SessionState, SignalConfig, SignalEvent, SignalMonitor,
    SignalSample, SystemSignals, SystemSignalsPort,
};

struct State {
    now_ms: u64,
    last_input_ms: u64,
    idle_fails: bool,
    power: Option<PowerSnapshot>,
    fullscreen: Option<FullscreenProbe>,
    session: Option<SessionState>,
    monitor: SignalMonitor,
    next_sample_ms: u64,
    queue: VecDeque<SignalEvent>,
    dropped: usize,
}

impl State {
    fn sample(&mut self) {
        let sample = SignalSample {
            idle_ms: (!self.idle_fails).then(|| self.now_ms.saturating_sub(self.last_input_ms)),
            power: self.power,
            fullscreen: self.fullscreen.clone(),
            session: self.session,
        };
        for ev in self.monitor.observe(self.now_ms, sample) {
            if self.queue.len() >= MAX_QUEUED_SIGNALS {
                self.queue.pop_front();
                self.dropped += 1;
            }
            self.queue.push_back(ev);
        }
    }

    fn advance(&mut self, ms: u64) {
        let end = self.now_ms.saturating_add(ms);
        let step = self.monitor.config().poll_interval().as_millis();
        let step = u64::try_from(step).unwrap_or(1_000).max(1);
        while self.next_sample_ms <= end {
            self.now_ms = self.next_sample_ms;
            self.sample();
            self.next_sample_ms = self.next_sample_ms.saturating_add(step);
        }
        self.now_ms = end;
    }
}

/// Sygnały systemowe w pamięci.
pub struct FakeSignals {
    state: Mutex<State>,
}

impl std::fmt::Debug for FakeSignals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeSignals").finish_non_exhaustive()
    }
}

impl Default for FakeSignals {
    fn default() -> Self {
        Self::new(SignalConfig::default())
    }
}

impl FakeSignals {
    /// Atrapa: zegar 0, wejście w chwili 0, zasilanie sieciowe, bez pełnego ekranu, sesja aktywna.
    /// Pierwsza próbka przy pierwszym `advance` (chwila 0).
    pub fn new(cfg: SignalConfig) -> Self {
        Self {
            state: Mutex::new(State {
                now_ms: 0,
                last_input_ms: 0,
                idle_fails: false,
                power: Some(PowerSnapshot::AC),
                fullscreen: Some(FullscreenProbe::none()),
                session: Some(SessionState::Active),
                monitor: SignalMonitor::new(cfg),
                next_sample_ms: 0,
                queue: VecDeque::new(),
                dropped: 0,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Wirtualny czas (ms).
    pub fn now_ms(&self) -> u64 {
        self.lock().now_ms
    }

    /// Wejście użytkownika teraz (klawisz, ruch myszy).
    pub fn input(&self) {
        let mut s = self.lock();
        s.last_input_ms = s.now_ms;
    }

    /// Licznik bezczynności nieczytelny (`true`) albo znów działa.
    pub fn fail_idle(&self, fails: bool) {
        self.lock().idle_fails = fails;
    }

    /// Zasilanie (`None` = odczyt nieudany).
    pub fn set_power(&self, power: Option<PowerSnapshot>) {
        self.lock().power = power;
    }

    /// Pełny ekran (`None` = odczyt nieudany).
    pub fn set_fullscreen(&self, probe: Option<FullscreenProbe>) {
        self.lock().fullscreen = probe;
    }

    /// Sesja (`None` = odczyt nieudany).
    pub fn set_session(&self, session: Option<SessionState>) {
        self.lock().session = session;
    }

    /// Powiadomienie systemu (zasilanie, sesja): natychmiastowa próbka.
    pub fn notify(&self) {
        self.lock().sample();
    }

    /// Przesuwa zegar, próbkując co `poll_ms`.
    pub fn advance(&self, ms: u64) {
        self.lock().advance(ms);
    }

    /// Ile zdarzeń odrzucono przy przepełnionej kolejce.
    pub fn dropped(&self) -> usize {
        self.lock().dropped
    }
}

impl IdlePort for FakeSignals {
    fn idle_ms(&self) -> Result<u64, PlatformError> {
        let s = self.lock();
        if s.idle_fails {
            return Err(PlatformError::Unsupported(
                "licznik bezczynności (atrapa)".into(),
            ));
        }
        Ok(s.now_ms.saturating_sub(s.last_input_ms))
    }
}

impl PowerPort for FakeSignals {
    fn power(&self) -> Result<PowerSnapshot, PlatformError> {
        self.lock()
            .power
            .ok_or_else(|| PlatformError::Unsupported("zasilanie (atrapa)".into()))
    }
}

impl FullscreenPort for FakeSignals {
    fn probe(&self) -> Result<FullscreenProbe, PlatformError> {
        self.lock()
            .fullscreen
            .clone()
            .ok_or_else(|| PlatformError::Unsupported("pełny ekran (atrapa)".into()))
    }
}

impl SessionPort for FakeSignals {
    fn session(&self) -> Result<SessionState, PlatformError> {
        self.lock()
            .session
            .ok_or_else(|| PlatformError::Unsupported("sesja (atrapa)".into()))
    }
}

impl SystemSignalsPort for FakeSignals {
    fn snapshot(&self) -> SystemSignals {
        self.lock().monitor.snapshot()
    }

    fn drain_events(&self) -> Vec<SignalEvent> {
        self.lock().queue.drain(..).collect()
    }

    /// Wirtualny czas: przesuwa zegar krokami próbkowania, aż pojawi się zdarzenie albo minie
    /// `timeout`.
    fn wait_events(&self, timeout: Duration) -> Vec<SignalEvent> {
        let mut s = self.lock();
        let end = s
            .now_ms
            .saturating_add(u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX));
        while s.queue.is_empty() && s.now_ms < end {
            let step = s.next_sample_ms.max(s.now_ms + 1).min(end) - s.now_ms;
            s.advance(step);
        }
        s.queue.drain(..).collect()
    }
}
