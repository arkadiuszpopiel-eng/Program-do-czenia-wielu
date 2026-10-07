//! Poza Windows: zapytania `Unsupported`, monitor i obserwacja się nie uruchamiają — logikę
//! (histereza, debounce, przeskanowanie, deny-lista) testuje się na `platform-fake`.

use platform_contract::PlatformError;

fn unsupported<T>(what: &str) -> Result<T, PlatformError> {
    Err(PlatformError::Unsupported(format!("{what}: tylko Windows")))
}

pub(crate) mod sys {
    use platform_contract::{FullscreenProbe, PlatformError, PowerSnapshot, SessionState};

    use super::unsupported;

    pub(crate) fn idle_ms() -> Result<u64, PlatformError> {
        unsupported("licznik bezczynności")
    }

    pub(crate) fn power() -> Result<PowerSnapshot, PlatformError> {
        unsupported("stan zasilania")
    }

    pub(crate) fn fullscreen(_own_pid: u32) -> Result<FullscreenProbe, PlatformError> {
        unsupported("tryb pełnego ekranu")
    }

    pub(crate) fn session() -> Result<SessionState, PlatformError> {
        unsupported("stan sesji")
    }
}

pub(crate) mod monitor {
    use std::sync::Arc;
    use std::time::Duration;

    use platform_contract::PlatformError;

    use super::unsupported;
    use crate::signals::Shared;

    /// Brak wątku monitora.
    pub(crate) struct Monitor;

    impl Monitor {
        pub(crate) fn start(_: Arc<Shared>, _: Duration) -> Result<Self, PlatformError> {
            unsupported("monitor sygnałów")
        }
    }
}

pub(crate) mod watcher {
    use std::path::Path;
    use std::sync::Arc;

    use platform_contract::{PlatformError, WatchId, WatchSpec};

    use super::unsupported;
    use crate::watch::Shared;

    /// Brak wątku obserwacji.
    pub(crate) struct Watcher;

    impl Watcher {
        pub(crate) fn start(
            _: &Arc<Shared>,
            _: WatchSpec,
            _: &Path,
            _: usize,
        ) -> Result<(WatchId, Self), PlatformError> {
            unsupported("obserwacja katalogów")
        }
    }
}
