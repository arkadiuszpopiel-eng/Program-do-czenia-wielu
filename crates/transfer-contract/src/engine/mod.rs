//! Silnik modułu `transfer` wspólny dla `-impl` (kontener ZIP + szyfrowanie) i `-fake` (paczki
//! w pamięci): eksport z portów do [`PackageSink`], plan importu (dry-run), snapshot dotkniętych
//! elementów, zapis per element i rollback. Kontener odpowiada za format pliku, sumy, limity
//! i szyfrowanie; silnik — za semantykę elementów.

mod apply;
mod export;
mod plan;
pub mod sessions;

use accounts_hub_contract::{SecretName, SecretString};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sessions_contract::{PortableSession, SessionError, SessionId};

pub use apply::rollback_items;
pub use export::{ExportOutcome, ExportSpec};
pub use plan::ImportPlan;

use crate::error::TransferError;
use crate::ports::{PackageSource, TransferPorts};
use crate::report::ItemRef;

/// Prefiks nazw sekretów modułu `transfer` (klucz snapshotów) — nigdy nie eksportowane.
pub const OWN_SECRETS_PREFIX: &str = "transfer/";

/// Nazwa sekretu z kluczem szyfrowania snapshotów tej maszyny.
pub const SNAPSHOT_KEY_SECRET: &str = "transfer/snapshot-key";

/// `rollback.json` w snapshocie.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RollbackData {
    /// Wersja rekordu.
    pub v: u32,
    /// Elementy utworzone przez import (rollback je usuwa).
    pub created: Vec<ItemRef>,
    /// Elementy zapisane w snapshocie (rollback je przywraca).
    pub saved: Vec<ItemRef>,
}

/// Silnik nad portami.
pub struct Engine<'a> {
    ports: &'a TransferPorts,
}

impl<'a> Engine<'a> {
    /// Silnik dla portów.
    pub fn new(ports: &'a TransferPorts) -> Self {
        Self { ports }
    }

    /// Porty.
    pub fn ports(&self) -> &TransferPorts {
        self.ports
    }

    /// Sesja lokalna w postaci przenośnej; `None`, gdy nie istnieje.
    pub fn local_session(&self, id: &SessionId) -> Result<Option<PortableSession>, TransferError> {
        let sessions = self.ports.sessions()?;
        let meta = match sessions.session(id) {
            Ok(meta) => meta,
            Err(SessionError::NotFound { .. }) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        Ok(Some(PortableSession {
            meta,
            turns: sessions.all_turns(id)?,
            active_leaf: sessions.active_leaf(id)?,
            draft: sessions.draft(id)?,
        }))
    }

    /// Wszystkie sekrety magazynu poza własnymi (`transfer/…`) — tylko jako wzorce dla strażnika
    /// zwykłych paczek (wartości nigdy nie trafiają do paczki).
    pub(crate) fn read_secrets(&self) -> Result<Vec<(SecretName, SecretString)>, TransferError> {
        let Some(store) = &self.ports.secrets else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for name in store.list()? {
            if name.as_str().starts_with(OWN_SECRETS_PREFIX) {
                continue;
            }
            if let Some(value) = store.get(&name)? {
                out.push((name, value));
            }
        }
        Ok(out)
    }
}

/// Odczytuje sesję z paczki (nagłówek + tury, z migracją).
pub(crate) fn read_package_session(
    ports: &TransferPorts,
    source: &mut dyn PackageSource,
    id: &SessionId,
) -> Result<(PortableSession, Vec<crate::report::UpcastStep>), TransferError> {
    let header_path = crate::paths::session_path(id, crate::paths::SESSION_FILE);
    let turns_path = crate::paths::session_path(id, crate::paths::TURNS_FILE);
    let header = source
        .read(&header_path)?
        .ok_or_else(|| TransferError::corrupt(format!("brak `{header_path}`")))?;
    let turns = source.read(&turns_path)?.unwrap_or_default();
    crate::portable::decode_session(id, &header, &turns, ports.workdir_root.as_deref())
}
