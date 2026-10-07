//! Sygnały systemowe (bezczynność, zasilanie, tryb gry, blokada sesji) i obserwacja katalogów
//! z `platform-windows-sys-impl` (albo atrapy z opcji) → okna zadań schedulera, Strażniczka
//! pamięci, cykl Ulepszacza w bezczynności i wyzwalacze plikowe. Poza Windows monitor się nie
//! uruchamia: „nigdy bezczynny" (bezpieczniej — praca w tle nie startuje sama).

use std::sync::Arc;

use app_memory::guardian::IdleSource;
use platform_contract::{DirWatchPort, SignalConfig, SystemSignals, SystemSignalsPort};

use crate::options::AppOptions;

/// Monitor sygnałów: z opcji albo Windows (uruchomiony); `None` — brak monitora.
pub(crate) fn signals(options: &AppOptions) -> Option<Arc<dyn SystemSignalsPort>> {
    if let Some(port) = &options.signals {
        return Some(port.clone());
    }
    let monitor = platform_windows_sys_impl::WinSignals::new(SignalConfig::default());
    match monitor.start() {
        Ok(()) => Some(Arc::new(monitor)),
        Err(e) => {
            tracing::info!(error = %e, "monitor sygnałów systemowych niedostępny");
            None
        }
    }
}

/// Obserwacja katalogów: z opcji albo Windows; poza Windows — brak.
pub(crate) fn dir_watch(options: &AppOptions) -> Option<Arc<dyn DirWatchPort>> {
    if let Some(port) = &options.dir_watch {
        return Some(port.clone());
    }
    cfg!(windows).then(|| {
        Arc::new(platform_windows_sys_impl::WinDirWatch::default()) as Arc<dyn DirWatchPort>
    })
}

/// Stan sygnałów (bez monitora — domyślny: aktywny użytkownik, bez gry, zasilanie nieznane).
pub(crate) fn snapshot(port: Option<&Arc<dyn SystemSignalsPort>>) -> SystemSignals {
    port.map(|p| p.snapshot()).unwrap_or_default()
}

/// Bezczynność dla Strażniczki pamięci z monitora sygnałów.
pub(crate) struct PortIdle(pub Arc<dyn SystemSignalsPort>);

impl IdleSource for PortIdle {
    fn idle_secs(&self) -> u64 {
        let s = self.0.snapshot();
        // Zablokowana stacja = nieobecność (bezczynność bez względu na licznik wejścia).
        if s.locked() {
            u64::MAX / 2
        } else {
            s.idle_secs()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(SystemSignals);

    impl SystemSignalsPort for Fixed {
        fn snapshot(&self) -> SystemSignals {
            self.0.clone()
        }
        fn drain_events(&self) -> Vec<platform_contract::SignalEvent> {
            Vec::new()
        }
        fn wait_events(&self, _t: std::time::Duration) -> Vec<platform_contract::SignalEvent> {
            Vec::new()
        }
    }

    #[test]
    fn idle_seconds_from_port_and_locked_session_counts_as_away() {
        let active = SystemSignals {
            idle_ms: 90_000,
            ..SystemSignals::default()
        };
        assert_eq!(PortIdle(Arc::new(Fixed(active.clone()))).idle_secs(), 90);
        let locked = SystemSignals {
            session: platform_contract::SessionState::Locked,
            ..active
        };
        assert!(PortIdle(Arc::new(Fixed(locked))).idle_secs() > 86_400);
        assert!(!snapshot(None).user_idle, "bez monitora — nigdy bezczynny");
    }
}
