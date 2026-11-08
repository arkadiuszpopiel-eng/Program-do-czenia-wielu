//! Circuit breaker per dostawca i reaktywna estymata okien limitów (429 + `retry-after`).
//! Czysta logika na zegarze w milisekundach (deterministyczna; czas podaje wywołujący).

use std::collections::VecDeque;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Konfiguracja obwodu (`[router.breaker]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BreakerConfig {
    /// Liczba błędów w oknie, po której obwód się otwiera.
    pub failures: u32,
    /// Okno liczenia błędów (ms).
    pub window_ms: u64,
    /// Czas otwarcia przed próbą half-open (ms).
    pub cooldown_ms: u64,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failures: 3,
            window_ms: 60_000,
            cooldown_ms: 60_000,
        }
    }
}

/// Stan obwodu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum BreakerState {
    /// Ruch normalny.
    Closed,
    /// Omijany do chwili `until_ms`.
    Open {
        /// Koniec otwarcia (ms zegara Routera).
        until_ms: u64,
    },
    /// Po odczekaniu: dopuszczona jedna próba.
    HalfOpen {
        /// Czy próba już trwa (kolejne żądania omijają dostawcę).
        trial_in_flight: bool,
    },
}

/// Przejście stanu (dla zdarzeń `router.breaker.*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "transition", rename_all = "snake_case")]
pub enum BreakerTransition {
    /// Obwód otwarty.
    Opened {
        /// Do kiedy.
        until_ms: u64,
    },
    /// Rozpoczęta próba half-open.
    HalfOpened,
    /// Obwód zamknięty po udanej próbie.
    Closed,
}

/// Obwód jednego dostawcy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CircuitBreaker {
    config: BreakerConfig,
    failures: VecDeque<u64>,
    open_until: Option<u64>,
    trial_in_flight: bool,
}

impl CircuitBreaker {
    /// Nowy, zamknięty obwód.
    pub fn new(config: BreakerConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    /// Stan w chwili `now_ms`.
    pub fn state(&self, now_ms: u64) -> BreakerState {
        match self.open_until {
            None => BreakerState::Closed,
            Some(until_ms) if now_ms < until_ms => BreakerState::Open { until_ms },
            Some(_) => BreakerState::HalfOpen {
                trial_in_flight: self.trial_in_flight,
            },
        }
    }

    /// `Ok` = można próbować; `Err(ms)` = za ile ms (0 = trwa próba half-open).
    pub fn admits(&self, now_ms: u64) -> Result<(), u64> {
        match self.state(now_ms) {
            BreakerState::Closed
            | BreakerState::HalfOpen {
                trial_in_flight: false,
            } => Ok(()),
            BreakerState::Open { until_ms } => Err(until_ms - now_ms),
            BreakerState::HalfOpen {
                trial_in_flight: true,
            } => Err(0),
        }
    }

    /// Początek wywołania: w stanie half-open zajmuje jedyną próbę.
    pub fn begin_attempt(&mut self, now_ms: u64) -> Option<BreakerTransition> {
        let half_open = matches!(
            self.state(now_ms),
            BreakerState::HalfOpen {
                trial_in_flight: false
            }
        );
        half_open.then(|| {
            self.trial_in_flight = true;
            BreakerTransition::HalfOpened
        })
    }

    /// Sukces: zamyka obwód i zeruje licznik.
    pub fn on_success(&mut self) -> Option<BreakerTransition> {
        self.failures.clear();
        self.trial_in_flight = false;
        self.open_until.take().map(|_| BreakerTransition::Closed)
    }

    /// Anulowanie: zwalnia próbę half-open bez oceny.
    pub fn on_cancel(&mut self) {
        self.trial_in_flight = false;
    }

    /// Błąd: w oknie `failures` błędów → otwarcie; nieudana próba half-open → ponowne otwarcie.
    pub fn on_failure(&mut self, now_ms: u64) -> Option<BreakerTransition> {
        match self.state(now_ms) {
            BreakerState::Open { .. } => None,
            BreakerState::HalfOpen { .. } => Some(self.open(now_ms)),
            BreakerState::Closed => {
                self.failures.push_back(now_ms);
                let from = now_ms.saturating_sub(self.config.window_ms);
                while self.failures.front().is_some_and(|t| *t < from) {
                    self.failures.pop_front();
                }
                let threshold = usize::try_from(self.config.failures.max(1)).unwrap_or(1);
                (self.failures.len() >= threshold).then(|| self.open(now_ms))
            }
        }
    }

    fn open(&mut self, now_ms: u64) -> BreakerTransition {
        let until_ms = now_ms.saturating_add(self.config.cooldown_ms);
        self.open_until = Some(until_ms);
        self.trial_in_flight = false;
        self.failures.clear();
        BreakerTransition::Opened { until_ms }
    }
}

/// Estymata okna limitu (plany/limity dostawców): z `retry-after` albo wykładniczo od 30 s do 1 h.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlanWindow {
    blocked_until: Option<u64>,
    streak: u32,
}

/// Pierwsza estymata bez `retry-after` (ms).
pub const PLAN_WINDOW_BASE_MS: u64 = 30_000;
/// Maksymalna estymata (ms).
pub const PLAN_WINDOW_MAX_MS: u64 = 3_600_000;

impl PlanWindow {
    /// 429: blokada do `now + retry_after` albo estymaty; zwraca koniec blokady.
    pub fn on_rate_limited(&mut self, now_ms: u64, retry_after_ms: Option<u64>) -> u64 {
        self.streak = self.streak.saturating_add(1);
        let estimate = PLAN_WINDOW_BASE_MS
            .saturating_mul(1u64 << self.streak.saturating_sub(1).min(16))
            .min(PLAN_WINDOW_MAX_MS);
        let until = now_ms.saturating_add(retry_after_ms.unwrap_or(estimate));
        let until = self.blocked_until.map_or(until, |u| u.max(until));
        self.blocked_until = Some(until);
        until
    }

    /// Sukces: okno odnowione.
    pub fn on_success(&mut self) {
        *self = Self::default();
    }

    /// Ile ms do odnowienia (`None` = nie blokuje).
    pub fn remaining(&self, now_ms: u64) -> Option<u64> {
        self.blocked_until
            .filter(|u| now_ms < *u)
            .map(|u| u - now_ms)
    }
}
