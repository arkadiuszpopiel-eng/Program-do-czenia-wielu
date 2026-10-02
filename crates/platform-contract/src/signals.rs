//! Sygnały systemowe razem: bezczynność, zasilanie, tryb gry, blokada sesji. [`SignalMonitor`]
//! to czysta maszyna stanów (histereza, filtr zmian) wspólna dla monitora Windows
//! (`platform-windows-sys-impl`, wątek z powiadomieniami + próbkowaniem) i atrapy z wirtualnym
//! zegarem (`platform-fake`). Konsumenci (podpina `app-*`): okna schedulera
//! (`SystemConditions`), Strażniczka pamięci (`IdleSource`, `HostConditions`), `model-residency`
//! (`ModeSource`), głos i computer use (blokada sesji).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::idle::{IdleConfig, IdleTracker, IdleTransition};
use crate::power::{DEFAULT_PERCENT_STEP, PowerSnapshot};
use crate::presence::{
    DEFAULT_GAME_EXIT_AFTER_MS, FullscreenProbe, GameModeTracker, GameReason, SessionState,
};

/// Zdarzenie: użytkownik bezczynny.
pub const EVENT_IDLE_ENTERED: &str = "platform.idle.entered";
/// Zdarzenie: użytkownik wrócił.
pub const EVENT_IDLE_EXITED: &str = "platform.idle.exited";
/// Zdarzenie: zmiana zasilania.
pub const EVENT_POWER_CHANGED: &str = "platform.power.changed";
/// Zdarzenie: tryb gry / pełny ekran włączony albo wyłączony.
pub const EVENT_FULLSCREEN_CHANGED: &str = "platform.fullscreen.changed";
/// Zdarzenie: stacja zablokowana albo sesja rozłączona.
pub const EVENT_SESSION_LOCKED: &str = "platform.session.locked";
/// Zdarzenie: sesja odblokowana.
pub const EVENT_SESSION_UNLOCKED: &str = "platform.session.unlocked";
/// Domyślny okres próbkowania monitora (bezczynność i pełny ekran nie mają powiadomień).
pub const DEFAULT_POLL_MS: u64 = 1_000;
/// Najwięcej zdarzeń w kolejce monitora (nadmiar: najstarsze odrzucane).
pub const MAX_QUEUED_SIGNALS: usize = 256;

/// Zbiorczy stan maszyny i użytkownika.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemSignals {
    /// Ostatni odczyt licznika bezczynności (ms; 0 = nieodczytany).
    pub idle_ms: u64,
    /// Użytkownik bezczynny (po histerezie).
    pub user_idle: bool,
    /// Zasilanie.
    pub power: PowerSnapshot,
    /// Tryb gry (po histerezie): powód albo `None`.
    pub game: Option<GameReason>,
    /// Sesja.
    pub session: SessionState,
    /// Chwila ostatniej próbki (zegar monitora, ms).
    pub sampled_at_ms: u64,
}

impl Default for SystemSignals {
    /// Przed pierwszą próbką: aktywny użytkownik, zasilanie nieznane, bez gry, sesja aktywna.
    fn default() -> Self {
        Self {
            idle_ms: 0,
            user_idle: false,
            power: PowerSnapshot::UNKNOWN,
            game: None,
            session: SessionState::Active,
            sampled_at_ms: 0,
        }
    }
}

impl SystemSignals {
    /// Tryb gry / pełny ekran.
    pub fn game_mode(&self) -> bool {
        self.game.is_some()
    }

    /// Na baterii.
    pub fn on_battery(&self) -> bool {
        self.power.on_battery()
    }

    /// Użytkownik nieobecny przy ekranie (blokada albo rozłączenie).
    pub fn locked(&self) -> bool {
        self.session.is_away()
    }

    /// Sekundy bez wejścia (dla `IdleSource` Strażniczki).
    pub fn idle_secs(&self) -> u64 {
        self.idle_ms / 1_000
    }
}

/// Zmiana sygnału (ładunek zdarzenia magistrali; publikuje `app-*`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "signal", rename_all = "snake_case")]
pub enum SignalEvent {
    /// Użytkownik bezczynny.
    IdleEntered {
        /// Licznik bezczynności (ms).
        idle_ms: u64,
    },
    /// Użytkownik wrócił.
    IdleExited {
        /// Jak długo trwała bezczynność (ms).
        idle_for_ms: u64,
    },
    /// Istotna zmiana zasilania.
    PowerChanged {
        /// Nowy stan.
        power: PowerSnapshot,
    },
    /// Tryb gry włączony (`reason`) albo wyłączony (`None`).
    GameModeChanged {
        /// Powód.
        reason: Option<GameReason>,
    },
    /// Zmiana stanu sesji.
    SessionChanged {
        /// Nowy stan.
        state: SessionState,
    },
}

impl SignalEvent {
    /// Nazwa zdarzenia na magistrali.
    pub fn name(&self) -> &'static str {
        match self {
            Self::IdleEntered { .. } => EVENT_IDLE_ENTERED,
            Self::IdleExited { .. } => EVENT_IDLE_EXITED,
            Self::PowerChanged { .. } => EVENT_POWER_CHANGED,
            Self::GameModeChanged { .. } => EVENT_FULLSCREEN_CHANGED,
            Self::SessionChanged { state } if state.is_away() => EVENT_SESSION_LOCKED,
            Self::SessionChanged { .. } => EVENT_SESSION_UNLOCKED,
        }
    }
}

/// Jedna próbka portów; `None` = odczyt nieudany (stan bez zmian — nigdy domysł).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignalSample {
    /// Licznik bezczynności (ms).
    pub idle_ms: Option<u64>,
    /// Zasilanie.
    pub power: Option<PowerSnapshot>,
    /// Pełny ekran.
    pub fullscreen: Option<FullscreenProbe>,
    /// Sesja.
    pub session: Option<SessionState>,
}

/// Konfiguracja monitora (`[platform.signals]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalConfig {
    /// Histereza bezczynności.
    pub idle: IdleConfig,
    /// Utrzymanie trybu gry po zniknięciu pełnego ekranu (ms).
    pub game_exit_after_ms: u64,
    /// Krok poziomu baterii zgłaszany jako zmiana (pp).
    pub power_percent_step: u8,
    /// Okres próbkowania (ms).
    pub poll_ms: u64,
}

impl Default for SignalConfig {
    fn default() -> Self {
        Self {
            idle: IdleConfig::default(),
            game_exit_after_ms: DEFAULT_GAME_EXIT_AFTER_MS,
            power_percent_step: DEFAULT_PERCENT_STEP,
            poll_ms: DEFAULT_POLL_MS,
        }
    }
}

impl SignalConfig {
    /// Okres próbkowania (≥ 100 ms).
    pub fn poll_interval(&self) -> Duration {
        Duration::from_millis(self.poll_ms.max(100))
    }
}

/// Monitor sygnałów (czysta maszyna stanów).
#[derive(Debug, Clone)]
pub struct SignalMonitor {
    cfg: SignalConfig,
    idle: IdleTracker,
    game: GameModeTracker,
    state: SystemSignals,
    power_reported: Option<PowerSnapshot>,
}

impl SignalMonitor {
    /// Nowy monitor.
    pub fn new(cfg: SignalConfig) -> Self {
        Self {
            cfg,
            idle: IdleTracker::new(cfg.idle),
            game: GameModeTracker::new(cfg.game_exit_after_ms),
            state: SystemSignals::default(),
            power_reported: None,
        }
    }

    /// Konfiguracja.
    pub fn config(&self) -> SignalConfig {
        self.cfg
    }

    /// Bieżący stan.
    pub fn snapshot(&self) -> SystemSignals {
        self.state.clone()
    }

    /// Próbka w chwili `now_ms` (zegar monotoniczny monitora) → zmiany w kolejności:
    /// sesja, tryb gry, zasilanie, bezczynność. Pierwszy odczyt zasilania jest zgłaszany.
    pub fn observe(&mut self, now_ms: u64, sample: SignalSample) -> Vec<SignalEvent> {
        let mut out = Vec::new();
        self.state.sampled_at_ms = now_ms;
        if let Some(s) = sample.session
            && s != SessionState::Unknown
            && s != self.state.session
        {
            self.state.session = s;
            out.push(SignalEvent::SessionChanged { state: s });
        }
        // Odczyt nieudany (`None`): tryb gry trwa — nie kończy się z braku danych.
        match &sample.fullscreen {
            Some(probe) => {
                if let Some(active) = self.game.observe(now_ms, probe.game_reason()) {
                    self.state.game = active;
                    out.push(SignalEvent::GameModeChanged { reason: active });
                }
            }
            None => self.game.hold(now_ms),
        }
        if let Some(p) = sample.power {
            self.state.power = p;
            let notable = self
                .power_reported
                .is_none_or(|r| p.differs_notably(&r, self.cfg.power_percent_step));
            if notable {
                self.power_reported = Some(p);
                out.push(SignalEvent::PowerChanged { power: p });
            }
        }
        if let Some(idle_ms) = sample.idle_ms {
            self.state.idle_ms = idle_ms;
            match self.idle.observe(now_ms, idle_ms) {
                Some(IdleTransition::Entered { idle_ms }) => {
                    out.push(SignalEvent::IdleEntered { idle_ms });
                }
                Some(IdleTransition::Exited { idle_for_ms }) => {
                    out.push(SignalEvent::IdleExited { idle_for_ms });
                }
                None => {}
            }
            self.state.user_idle = self.idle.is_idle();
        }
        out
    }
}

/// Port sygnałów (monitor w tle; jeden konsument kolejki — pompa zdarzeń w `app-*`).
pub trait SystemSignalsPort: Send + Sync {
    /// Bieżący stan (bez czekania).
    fn snapshot(&self) -> SystemSignals;
    /// Zdarzenia od ostatniego odbioru (kolejka ≤ [`MAX_QUEUED_SIGNALS`]).
    fn drain_events(&self) -> Vec<SignalEvent>;
    /// Jak `drain_events`, ale czeka na zdarzenie co najwyżej `timeout`.
    fn wait_events(&self, timeout: Duration) -> Vec<SignalEvent>;
}
