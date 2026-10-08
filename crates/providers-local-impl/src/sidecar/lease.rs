//! Pomocnicze elementy startu sidecara: port i klucz API uruchomienia oraz słuchacz dzierżawy
//! `model-residency`.

use std::sync::Weak;
use std::sync::atomic::Ordering;
use std::time::Duration;

use model_residency_contract::{Lease, LeaseListener, Revocation};

use super::Sidecar;
use crate::error::LocalError;

/// Wolny port na `127.0.0.1` (system przydziela; serwer wiąże go chwilę później).
pub(super) fn free_port() -> Result<u16, LocalError> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

/// Losowy klucz API (192 bity) — per uruchomienie, tylko w pamięci.
pub(super) fn random_key() -> Result<String, LocalError> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|e| LocalError::Spawn(format!("losowanie klucza: {e}")))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Słuchacz dzierżawy: odebranie albo przeniesienie zwalnia sidecar, gdy jest bezczynny.
pub(super) struct Listener(pub(super) Weak<Sidecar>);

impl LeaseListener for Listener {
    fn revoked(&self, revocation: &Revocation) {
        self.release_now(&format!("dzierżawa odebrana: {:?}", revocation.reason));
    }

    fn moved(&self, _lease: &Lease) {
        self.release_now("tryb gry: przeładowanie na CPU przy następnym żądaniu");
    }
}

impl Listener {
    fn release_now(&self, reason: &str) {
        let Some(sidecar) = self.0.upgrade() else {
            return;
        };
        sidecar.revoked.store(true, Ordering::SeqCst);
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            let reason = reason.to_owned();
            rt.spawn(async move { sidecar.stop_if_idle(&reason).await });
        }
    }
}

/// Limit czasu startu serwera dla modelu o rozmiarze `size_mb`. Duży model ładuje się dłużej
/// (zimny dysk, skaner antywirusowy): do limitu z konfiguracji dochodzi 20 s na każdy pełny
/// gigabajt pliku (Bielik 4.5B Q8, ok. 4,7 GB: 60 s + 80 s).
pub(super) fn startup_limit(base: Duration, size_mb: u32) -> Duration {
    base + Duration::from_secs(u64::from(size_mb) / 1024 * 20)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R-08 (docs/STATUS.md): limit rośnie z rozmiarem modelu, a mały model zachowuje limit bazowy.
    #[test]
    fn startup_limit_grows_with_model_size() {
        let base = Duration::from_secs(60);
        assert_eq!(startup_limit(base, 0), base);
        assert_eq!(startup_limit(base, 900), base);
        assert_eq!(startup_limit(base, 1024), Duration::from_secs(80));
        assert_eq!(startup_limit(base, 4_800), Duration::from_secs(140));
        assert_eq!(
            startup_limit(base, u32::MAX),
            base + Duration::from_secs(u64::from(u32::MAX) / 1024 * 20)
        );
    }
}
