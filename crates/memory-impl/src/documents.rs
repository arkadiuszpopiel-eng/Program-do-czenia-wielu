//! Pamięć w paczce `.alfa`: adapter [`DocumentStore`] kategorii `memory` (`transfer`).
//!
//! Dokument = zakres (`global.ndjson`, `project/<id>.ndjson`, `agent/<id>.ndjson`,
//! `session/<id>.ndjson`), linia = wpis z proweniencją i historią wersji. **Pamięć sesji
//! prywatnych nie jest wystawiana do eksportu** (bezpieczniej: `DocumentStore` nie zna zgody na
//! eksport prywatnych). Zapis = import zakresu w trybie „dokładnie zawartość dokumentu” (scalanie
//! NDJSON robi silnik `transfer`); wiersz łamiący reguły (np. treść niezaufana w zakresie
//! szerszym) → cały dokument odrzucony, nic nie zapisano. Usunięcie = `forget` zakresu.

use std::sync::Arc;

use memory_contract::export::{decode_ndjson, document_name, encode_ndjson, scope_of_document};
use memory_contract::{
    Accessor, ForgetTarget, ImportPolicy, MemoryScope, MemoryService, PrivacyOracle, check_imported,
};
use transfer_contract::{DocumentStore, TransferError};

/// Adapter pamięci dla `transfer` (rejestrowany pod `Category::Memory`).
pub struct MemoryDocuments {
    memory: Arc<dyn MemoryService>,
    privacy: Arc<dyn PrivacyOracle>,
}

impl MemoryDocuments {
    /// Nowy adapter.
    pub fn new(memory: Arc<dyn MemoryService>, privacy: Arc<dyn PrivacyOracle>) -> Self {
        Self { memory, privacy }
    }

    fn exportable(&self, scope: &MemoryScope) -> bool {
        match scope {
            MemoryScope::Session(s) => !self.privacy.is_private(s),
            _ => true,
        }
    }

    fn scope(name: &str) -> Result<MemoryScope, TransferError> {
        scope_of_document(name)
            .ok_or_else(|| TransferError::invalid(name, "nazwa dokumentu pamięci spoza formatu"))
    }
}

fn port(err: impl std::fmt::Display) -> TransferError {
    TransferError::port("memory", err)
}

impl DocumentStore for MemoryDocuments {
    fn list(&self) -> Result<Vec<String>, TransferError> {
        let mut names: Vec<String> = self
            .memory
            .scopes(&Accessor::Owner)
            .map_err(port)?
            .into_iter()
            .filter(|s| s.entries > 0 && self.exportable(&s.scope))
            .map(|s| document_name(&s.scope))
            .collect();
        names.sort();
        Ok(names)
    }

    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, TransferError> {
        let Some(scope) = scope_of_document(name) else {
            return Ok(None);
        };
        if !self.exportable(&scope) {
            return Ok(None);
        }
        let entries = self
            .memory
            .export_scope(&Accessor::Owner, &scope, name)
            .map_err(port)?;
        if entries.is_empty() {
            return Ok(None);
        }
        encode_ndjson(&entries).map(Some).map_err(port)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), TransferError> {
        let scope = Self::scope(name)?;
        let entries = decode_ndjson(bytes).map_err(|e| TransferError::invalid(name, e))?;
        for e in &entries {
            if let Err(reason) = check_imported(&scope, e.clone()) {
                return Err(TransferError::invalid(
                    name,
                    format!("wpis {}: {reason}", e.id),
                ));
            }
        }
        self.memory
            .import_scope(&Accessor::Owner, &scope, entries, ImportPolicy::Replace)
            .map_err(port)?;
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<bool, TransferError> {
        let scope = Self::scope(name)?;
        let report = self
            .memory
            .forget_as(&Accessor::Owner, &ForgetTarget::Scope(scope))
            .map_err(port)?;
        Ok(!report.removed.is_empty() || !report.shredded.is_empty())
    }
}
