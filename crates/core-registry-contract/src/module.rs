//! Trait `Module` — wzorzec dla każdego crate'a `-impl`.

use std::sync::Arc;

use async_trait::async_trait;
use core_bus_contract::EventBus;

use crate::manifest::ModuleManifest;
use crate::refs::ModuleId;

/// Kontekst przekazywany modułowi przy starcie.
#[derive(Clone)]
pub struct ModuleContext {
    /// Identyfikator uruchamianego modułu (z manifestu).
    pub module_id: ModuleId,
    /// Magistrala zdarzeń jądra (jedyny kanał komunikacji między modułami).
    pub bus: Arc<dyn EventBus>,
}

impl ModuleContext {
    /// Nowy kontekst.
    pub fn new(module_id: ModuleId, bus: Arc<dyn EventBus>) -> Self {
        Self { module_id, bus }
    }
}

/// Stan zdrowia modułu raportowany rejestrowi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// Moduł działa poprawnie.
    Healthy,
    /// Moduł działa z ograniczeniami (opis).
    Degraded(String),
    /// Moduł nie działa (opis).
    Unhealthy(String),
    /// Moduł nie został jeszcze uruchomiony.
    NotStarted,
}

/// Błędy cyklu życia modułu.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ModuleError {
    /// Moduł jest już uruchomiony.
    #[error("moduł już uruchomiony")]
    AlreadyStarted,
    /// Moduł nie jest uruchomiony.
    #[error("moduł nie jest uruchomiony")]
    NotStarted,
    /// Brak wymaganego kontraktu w rejestrze.
    #[error("brak wymaganego kontraktu `{0}`")]
    MissingContract(String),
    /// Błąd specyficzny dla modułu.
    #[error("{0}")]
    Other(String),
}

/// Moduł zarządzany przez rejestr: manifest, start/stop, health-check.
#[async_trait]
pub trait Module: Send + Sync {
    /// Manifest modułu (statyczny opis z `module.toml`).
    fn manifest(&self) -> &ModuleManifest;

    /// Uruchamia moduł. Drugie wywołanie bez `stop` → `ModuleError::AlreadyStarted`.
    async fn start(&mut self, ctx: ModuleContext) -> Result<(), ModuleError>;

    /// Zatrzymuje moduł i zwalnia zasoby. Bez wcześniejszego `start` → `ModuleError::NotStarted`.
    async fn stop(&mut self) -> Result<(), ModuleError>;

    /// Bieżący stan zdrowia.
    fn health(&self) -> HealthStatus;
}
