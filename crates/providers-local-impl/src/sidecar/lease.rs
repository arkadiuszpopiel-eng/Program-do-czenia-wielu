//! Pomocnicze elementy startu sidecara: port i klucz API uruchomienia oraz słuchacz dzierżawy
//! `model-residency`.

use std::sync::Weak;
use std::sync::atomic::Ordering;

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
