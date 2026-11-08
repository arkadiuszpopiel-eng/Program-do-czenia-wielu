//! Bezczynność użytkownika (PLAN §9.3 okna „tylko w bezczynności”, §10 konsolidacja nocna):
//! port surowego licznika (Windows: `GetLastInputInfo` + `GetTickCount64`) i deterministyczny
//! tracker z histerezą — wspólny dla `platform-windows-sys-impl` i `platform-fake`.
//!
//! Histereza: wejście w bezczynność po `idle_after_ms` bez wejścia; wyjście dopiero, gdy
//! aktywność trwa co najmniej `wake_confirm_ms` (dwa wejścia w odstępie ≥ progu w oknie
//! `wake_window_ms`). Pojedyncze trącenie myszy nie przerywa zadań tła; licznik nieczytelny
//! (błąd portu) nie zmienia stanu.

use serde::{Deserialize, Serialize};

use crate::error::PlatformError;

/// Domyślny próg bezczynności (5 min).
pub const DEFAULT_IDLE_AFTER_MS: u64 = 5 * 60_000;
/// Domyślny czas potwierdzenia powrotu użytkownika.
pub const DEFAULT_WAKE_CONFIRM_MS: u64 = 1_000;
/// Domyślne okno potwierdzenia powrotu.
pub const DEFAULT_WAKE_WINDOW_MS: u64 = 10_000;

/// Port licznika bezczynności.
pub trait IdlePort: Send + Sync {
    /// Milisekundy od ostatniego wejścia użytkownika (klawiatura, mysz, dotyk) w bieżącej sesji.
    /// Uwaga (Windows): wejście syntetyczne (`SendInput`) też zeruje licznik — computer use
    /// Alfy wygląda jak aktywność (kierunek bezpieczny: zadania tła nie startują).
    fn idle_ms(&self) -> Result<u64, PlatformError>;
}

/// Progi histerezy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdleConfig {
    /// Po ilu ms bez wejścia użytkownik jest bezczynny.
    pub idle_after_ms: u64,
    /// Ile ms musi trwać aktywność, żeby wyjść z bezczynności (0 = pierwsze wejście).
    pub wake_confirm_ms: u64,
    /// Brak dalszego wejścia przez tyle ms po pierwszym = trącenie (zapominane).
    pub wake_window_ms: u64,
}

impl Default for IdleConfig {
    fn default() -> Self {
        Self {
            idle_after_ms: DEFAULT_IDLE_AFTER_MS,
            wake_confirm_ms: DEFAULT_WAKE_CONFIRM_MS,
            wake_window_ms: DEFAULT_WAKE_WINDOW_MS,
        }
    }
}

impl IdleConfig {
    /// Sprawdza progi: bezczynność ≥ 1 s, potwierdzenie mieści się w oknie.
    pub fn validate(&self) -> Result<(), PlatformError> {
        if self.idle_after_ms < 1_000 {
            return Err(PlatformError::Unsupported(
                "próg bezczynności musi wynosić co najmniej 1 s".into(),
            ));
        }
        if self.wake_confirm_ms > 0 && self.wake_confirm_ms >= self.wake_window_ms {
            return Err(PlatformError::Unsupported(
                "czas potwierdzenia powrotu musi być krótszy niż okno".into(),
            ));
        }
        Ok(())
    }
}

/// Przejście stanu bezczynności.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "transition", rename_all = "snake_case")]
pub enum IdleTransition {
    /// Użytkownik stał się bezczynny.
    Entered {
        /// Licznik bezczynności w chwili przejścia (ms).
        idle_ms: u64,
    },
    /// Użytkownik wrócił (aktywność potwierdzona).
    Exited {
        /// Jak długo trwała bezczynność (od ostatniego wejścia do pierwszego wejścia po powrocie).
        idle_for_ms: u64,
    },
}

/// Tracker bezczynności (czysta maszyna stanów; czas = zegar monotoniczny wywołującego, ms).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdleTracker {
    cfg: IdleConfig,
    idle: bool,
    idle_since: u64,
    last_input: u64,
    wake_first: Option<u64>,
}

impl IdleTracker {
    /// Nowy tracker (stan początkowy: aktywny — bezpieczniej dla zadań tła).
    pub fn new(cfg: IdleConfig) -> Self {
        Self {
            cfg,
            idle: false,
            idle_since: 0,
            last_input: 0,
            wake_first: None,
        }
    }

    /// Progi.
    pub fn config(&self) -> IdleConfig {
        self.cfg
    }

    /// Czy użytkownik jest bezczynny (po histerezie).
    pub fn is_idle(&self) -> bool {
        self.idle
    }

    /// Próbka: chwila `now_ms` i licznik bezczynności z portu. Zwraca przejście, jeśli nastąpiło.
    pub fn observe(&mut self, now_ms: u64, idle_ms: u64) -> Option<IdleTransition> {
        let input_at = now_ms.saturating_sub(idle_ms);
        if !self.idle {
            if idle_ms >= self.cfg.idle_after_ms {
                self.idle = true;
                self.idle_since = input_at;
                self.last_input = input_at;
                self.wake_first = None;
                return Some(IdleTransition::Entered { idle_ms });
            }
            return None;
        }
        if input_at <= self.last_input {
            if self
                .wake_first
                .is_some_and(|f| now_ms.saturating_sub(f) > self.cfg.wake_window_ms)
            {
                self.wake_first = None;
            }
            return None;
        }
        self.last_input = input_at;
        let first = match self.wake_first {
            Some(f) if input_at.saturating_sub(f) <= self.cfg.wake_window_ms => f,
            _ => input_at,
        };
        self.wake_first = Some(first);
        if input_at.saturating_sub(first) >= self.cfg.wake_confirm_ms {
            self.idle = false;
            self.wake_first = None;
            return Some(IdleTransition::Exited {
                idle_for_ms: first.saturating_sub(self.idle_since),
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_validation() {
        assert!(IdleConfig::default().validate().is_ok());
        let short = IdleConfig {
            idle_after_ms: 10,
            ..IdleConfig::default()
        };
        assert!(short.validate().is_err());
        let wide = IdleConfig {
            wake_confirm_ms: 20_000,
            ..IdleConfig::default()
        };
        assert!(wide.validate().is_err());
        let immediate = IdleConfig {
            wake_confirm_ms: 0,
            wake_window_ms: 0,
            ..IdleConfig::default()
        };
        assert!(immediate.validate().is_ok());
    }

    #[test]
    fn immediate_wake_when_confirm_is_zero() {
        let mut t = IdleTracker::new(IdleConfig {
            idle_after_ms: 1_000,
            wake_confirm_ms: 0,
            wake_window_ms: 0,
        });
        assert_eq!(
            t.observe(5_000, 1_000),
            Some(IdleTransition::Entered { idle_ms: 1_000 })
        );
        assert_eq!(
            t.observe(6_000, 10),
            Some(IdleTransition::Exited { idle_for_ms: 1_990 })
        );
        assert!(!t.is_idle());
    }
}
