//! Konfiguracja rejestru i zegar (wirtualny w testach).

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use chrono::{DateTime, Utc};
use core_registry_contract::{ContractRef, ModuleId};

/// Źródło czasu rejestru (bezczynność modułów). W testach — zegar wirtualny.
pub trait Clock: Send + Sync {
    /// Bieżący czas.
    fn now(&self) -> DateTime<Utc>;
}

/// Zegar systemowy.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

impl<F: Fn() -> DateTime<Utc> + Send + Sync> Clock for F {
    fn now(&self) -> DateTime<Utc> {
        self()
    }
}

/// Kontrakty jądra dostępne bez modułu w rejestrze (spełniają `requires`).
pub const KERNEL_CONTRACTS: [&str; 4] = [
    "core-bus-contract",
    "core-registry-contract",
    "core-config-contract",
    "core-log-contract",
];

/// Ustawienia rejestru (`[core.registry]` w konfiguracji, SPEC core-registry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryConfig {
    /// Domyślny limit bezczynności modułów `lazy`/`on-demand` (`idle_unload_default = "10m"`).
    pub idle_unload: Duration,
    /// Limity bezczynności per moduł (nadpisują domyślny).
    pub idle_overrides: BTreeMap<ModuleId, Duration>,
    /// Liczba nieudanych prób startu, po której rejestr odmawia kolejnych (`crash_loop_limit`).
    pub crash_loop_limit: u8,
    /// Kontrakty dostarczane przez jądro poza rejestrem.
    pub external_contracts: BTreeSet<ContractRef>,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            idle_unload: Duration::from_secs(600),
            idle_overrides: BTreeMap::new(),
            crash_loop_limit: 3,
            external_contracts: KERNEL_CONTRACTS
                .iter()
                .map(|name| ContractRef {
                    name: (*name).to_owned(),
                    major: 1,
                })
                .collect(),
        }
    }
}

impl RegistryConfig {
    /// Limit bezczynności dla modułu.
    pub fn idle_limit(&self, id: &ModuleId) -> Duration {
        self.idle_overrides
            .get(id)
            .copied()
            .unwrap_or(self.idle_unload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = RegistryConfig::default();
        assert_eq!(c.idle_unload, Duration::from_secs(600));
        assert_eq!(c.crash_loop_limit, 3);
        assert!(
            c.external_contracts
                .contains(&"core-bus-contract@1".parse().unwrap())
        );
        let id = ModuleId::new("voice-stt").unwrap();
        let mut c2 = c.clone();
        c2.idle_overrides.insert(id.clone(), Duration::from_secs(5));
        assert_eq!(c2.idle_limit(&id), Duration::from_secs(5));
        assert_eq!(c.idle_limit(&id), Duration::from_secs(600));
        let fixed = || DateTime::<Utc>::from_timestamp(7, 0).unwrap();
        assert_eq!(fixed.now().timestamp(), 7);
        assert!(SystemClock.now().timestamp() > 0);
    }
}
