//! Umiejętności w paczce `.alfa` (`transfer`, kategoria `skills`): jeden dokument
//! [`BUNDLE_DOCUMENT`] = paczka `alfa.skills.v1` z SHA-256 zainstalowanych umiejętności.
//! Zapis dokumentu (import) **nigdy nie instaluje** — umiejętności trafiają do biblioteki jako
//! propozycje z importu własnej paczki (zatwierdza właściciel). Usunięcie dokumentu niczego nie
//! usuwa z biblioteki (bezpieczniej: tryb „zastąp” importu nie kasuje umiejętności).

use std::sync::Arc;

use skills_contract::{BUNDLE_DOCUMENT, ImportOrigin, SkillBundle, SkillState, Skills};
use transfer_contract::{DocumentStore, TransferError};

use crate::SkillsModule;

/// Adapter `DocumentStore` biblioteki umiejętności.
pub struct SkillsDocuments {
    module: Arc<SkillsModule>,
}

impl SkillsDocuments {
    /// Adapter nad modułem.
    pub fn new(module: Arc<SkillsModule>) -> Self {
        Self { module }
    }
}

fn port(e: impl std::fmt::Display) -> TransferError {
    TransferError::Port {
        port: "skills".into(),
        reason: e.to_string(),
    }
}

impl DocumentStore for SkillsDocuments {
    fn list(&self) -> Result<Vec<String>, TransferError> {
        let any = self
            .module
            .list()
            .iter()
            .any(|r| r.state == SkillState::Installed);
        Ok(if any {
            vec![BUNDLE_DOCUMENT.to_owned()]
        } else {
            Vec::new()
        })
    }

    fn read(&self, name: &str) -> Result<Option<Vec<u8>>, TransferError> {
        if name != BUNDLE_DOCUMENT {
            return Ok(None);
        }
        let bundle = self.module.export(&[]).map_err(port)?;
        bundle.to_bytes().map(Some).map_err(port)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), TransferError> {
        if name != BUNDLE_DOCUMENT {
            return Err(TransferError::Invalid {
                path: name.to_owned(),
                reason: "nieznany dokument umiejętności".into(),
            });
        }
        let bundle = SkillBundle::from_bytes(bytes).map_err(port)?;
        let (_, events, bus) = self
            .module
            .mutate(|l, now| l.import(&bundle, ImportOrigin::OwnPackage, now))
            .map_err(port)?;
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                for e in events {
                    let _ = bus.publish(e).await;
                }
            });
        }
        Ok(())
    }

    fn remove(&self, _name: &str) -> Result<bool, TransferError> {
        Ok(false)
    }
}
