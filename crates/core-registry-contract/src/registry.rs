//! Trait `Registry` — rejestr modułów: rejestracja, graf kontraktów, cykl życia, zdarzenia.

use std::fmt;

use async_trait::async_trait;
use core_bus_contract::EventKind;
use serde::{Deserialize, Serialize};

use crate::manifest::Lifecycle;
use crate::module::{HealthStatus, Module};
use crate::refs::{ContractRef, ModuleId};

/// Zdarzenie zmiany stanu modułu; ładunek `{module, from, to, reason?}`.
pub const EVENT_STATE_CHANGED: &str = "registry.module.state_changed";
/// Zdarzenie wyniku health-checku; ładunek `{module, status, detail?}`.
pub const EVENT_HEALTH: &str = "registry.module.health";
/// Zdarzenie nieudanego rozwiązania kontraktu; ładunek `{contract, reason}`.
pub const EVENT_RESOLVE_FAILED: &str = "registry.resolve_failed";

/// Rodzaj zdarzenia rejestru jako `EventKind` (np. `registry_event_kind(EVENT_HEALTH)`).
pub fn registry_event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Stan modułu w rejestrze (SPEC core-registry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum ModuleState {
    /// Wyłączony przez użytkownika; nie jest dostawcą kontraktów.
    Disabled,
    /// Zarejestrowany, nieuruchomiony (stan początkowy i po zwolnieniu).
    Unloaded,
    /// W trakcie startu.
    Loading,
    /// Uruchomiony i zdrowy.
    Ready,
    /// Uruchomiony, działa z ograniczeniami.
    Degraded {
        /// Opis ograniczenia.
        reason: String,
    },
    /// Start nie powiódł się (licznik nieudanych prób od ostatniego udanego startu).
    Failed {
        /// Liczba nieudanych prób.
        restarts: u8,
        /// Powód ostatniej porażki.
        reason: String,
    },
}

impl ModuleState {
    /// Czy moduł jest uruchomiony (`Ready` lub `Degraded`).
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Ready | Self::Degraded { .. })
    }

    /// Krótka nazwa stanu (jak w JSON), np. `"ready"`.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Unloaded => "unloaded",
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Degraded { .. } => "degraded",
            Self::Failed { .. } => "failed",
        }
    }
}

/// Migawka stanu modułu (lista na stronie Ustawienia → Moduły).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleStatus {
    /// Identyfikator modułu.
    pub id: ModuleId,
    /// Wersja z manifestu.
    pub version: semver::Version,
    /// Cykl życia z manifestu.
    pub lifecycle: Lifecycle,
    /// Bieżący stan.
    pub state: ModuleState,
    /// Kontrakty dostarczane.
    pub provides: Vec<ContractRef>,
}

/// Lista identyfikatorów jako `a → b → c` (komunikaty błędów).
struct IdPath<'a>(&'a [ModuleId]);

impl fmt::Display for IdPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, id) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" → ")?;
            }
            write!(f, "{id}")?;
        }
        Ok(())
    }
}

fn path(ids: &[ModuleId]) -> IdPath<'_> {
    IdPath(ids)
}

/// Błędy rejestru. Każdy problem z grafem kończy się błędem, nigdy paniką.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// Moduł o tym `id` jest już zarejestrowany.
    #[error("moduł `{0}` jest już zarejestrowany")]
    Duplicate(ModuleId),
    /// Nieznany moduł.
    #[error("nieznany moduł `{0}`")]
    UnknownModule(ModuleId),
    /// Wymagany kontrakt nie ma dostawcy wśród włączonych modułów.
    #[error(
        "moduł `{module}` wymaga kontraktu `{contract}`, którego nie dostarcza żaden włączony moduł"
    )]
    MissingContract {
        /// Moduł wymagający.
        module: ModuleId,
        /// Brakujący kontrakt.
        contract: ContractRef,
    },
    /// Kontrakt ma więcej niż jednego dostawcę (lub dostarcza go też jądro).
    #[error("kontrakt `{contract}` ma wielu dostawców: {}", path(.modules))]
    ConflictingProviders {
        /// Kontrakt.
        contract: ContractRef,
        /// Moduły dostarczające (pusty wpis jądra pomijany).
        modules: Vec<ModuleId>,
    },
    /// Cykl w grafie zależności (ścieżka zamknięta: pierwszy = ostatni).
    #[error("cykl zależności kontraktów: {}", path(.0))]
    Cycle(Vec<ModuleId>),
    /// Żaden włączony moduł nie dostarcza kontraktu.
    #[error("żaden włączony moduł nie dostarcza kontraktu `{0}`")]
    NoProvider(ContractRef),
    /// Moduł `on-demand` nie został jawnie aktywowany.
    #[error("moduł `{0}` jest `on-demand` i nie został aktywowany")]
    NotActivated(ModuleId),
    /// Moduł jest wyłączony.
    #[error("moduł `{0}` jest wyłączony")]
    Disabled(ModuleId),
    /// Moduł `always` zatrzymuje wyłącznie zamknięcie jądra (lub watchdog).
    #[error("moduł `{0}` jest rezydentny (`always`); zatrzymuje go tylko zamknięcie jądra")]
    Resident(ModuleId),
    /// Moduł jest potrzebny innym (włączonym lub rezydentnym) modułom.
    #[error("moduł `{module}` jest wymagany przez: {}", path(.dependents))]
    InUse {
        /// Moduł.
        module: ModuleId,
        /// Moduły zależne.
        dependents: Vec<ModuleId>,
    },
    /// `Module::start` zwrócił błąd.
    #[error("start modułu `{module}` nie powiódł się: {reason}")]
    StartFailed {
        /// Moduł.
        module: ModuleId,
        /// Powód.
        reason: String,
    },
    /// Przekroczono limit prób startu (crash-loop); moduł pozostaje w `Failed`.
    #[error("moduł `{module}` przekroczył limit prób startu ({restarts})")]
    CrashLoop {
        /// Moduł.
        module: ModuleId,
        /// Liczba nieudanych prób.
        restarts: u8,
    },
    /// `Module::stop` zwrócił błąd.
    #[error("zatrzymanie modułu `{module}` nie powiodło się: {reason}")]
    StopFailed {
        /// Moduł.
        module: ModuleId,
        /// Powód.
        reason: String,
    },
}

/// Rejestr modułów: rejestracja manifestów, walidacja grafu kontraktów, kolejność startu,
/// cykl życia (`always` — przy `boot`, `lazy` — przy pierwszym `acquire`, `on-demand` — przy
/// `activate`), zwalnianie po bezczynności i zdarzenia `registry.*` na magistrali.
///
/// Start modułu zawsze najpierw uruchamia jego zależności (w kolejności topologicznej) —
/// potrzeba zależności jest „żądaniem”, także dla modułów `on-demand`.
#[async_trait]
pub trait Registry: Send + Sync {
    /// Rejestruje moduł (manifest + instancja). Duplikat `id` → `Duplicate`.
    async fn register(&self, module: Box<dyn Module>) -> Result<(), RegistryError>;

    /// Waliduje graf włączonych modułów i zwraca kolejność startu (zależności przed zależnymi,
    /// remisy leksykograficznie po `id`). Braki, konflikty i cykle → błąd.
    async fn start_order(&self) -> Result<Vec<ModuleId>, RegistryError>;

    /// Waliduje graf i uruchamia moduły `always` wraz z zależnościami; zwraca uruchomione.
    async fn boot(&self) -> Result<Vec<ModuleId>, RegistryError>;

    /// Jawnie uruchamia moduł (ścieżka `on-demand`) wraz z zależnościami.
    async fn activate(&self, id: &ModuleId) -> Result<(), RegistryError>;

    /// Pierwsze/kolejne użycie kontraktu: uruchamia dostawcę `lazy` (i zależności), odnotowuje
    /// użycie (licznik bezczynności) i zwraca `id` dostawcy. Nieaktywny `on-demand` → `NotActivated`.
    async fn acquire(&self, contract: &ContractRef) -> Result<ModuleId, RegistryError>;

    /// Zatrzymuje moduł; najpierw uruchomione moduły zależne. Zwraca zatrzymane w kolejności.
    async fn deactivate(&self, id: &ModuleId) -> Result<Vec<ModuleId>, RegistryError>;

    /// Włącza lub wyłącza moduł. Wyłączenie modułu wymaganego przez włączone → `InUse`.
    async fn set_enabled(&self, id: &ModuleId, enabled: bool) -> Result<(), RegistryError>;

    /// Migawki stanów wszystkich modułów (posortowane po `id`).
    async fn list(&self) -> Vec<ModuleStatus>;

    /// Health-check modułu (publikuje `registry.module.health`).
    async fn health(&self, id: &ModuleId) -> Result<HealthStatus, RegistryError>;

    /// Zwalnia moduły `lazy`/`on-demand` bezczynne dłużej niż limit i bez uruchomionych
    /// modułów zależnych. Zwraca zwolnione w kolejności zatrzymania.
    async fn unload_idle(&self) -> Result<Vec<ModuleId>, RegistryError>;

    /// Zatrzymuje wszystkie uruchomione moduły w odwrotnej kolejności startu.
    async fn shutdown(&self) -> Result<Vec<ModuleId>, RegistryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> ModuleId {
        ModuleId::new(s).unwrap()
    }

    #[test]
    fn state_names_and_running() {
        assert!(ModuleState::Ready.is_running());
        assert!(ModuleState::Degraded { reason: "x".into() }.is_running());
        assert!(!ModuleState::Loading.is_running());
        assert_eq!(
            ModuleState::Failed {
                restarts: 1,
                reason: "x".into()
            }
            .name(),
            "failed"
        );
        let json = serde_json::to_value(ModuleState::Unloaded).unwrap();
        assert_eq!(json, serde_json::json!({"state": "unloaded"}));
    }

    #[test]
    fn errors_render_paths() {
        let cycle = RegistryError::Cycle(vec![id("a"), id("b"), id("a")]);
        assert_eq!(cycle.to_string(), "cykl zależności kontraktów: a → b → a");
        let in_use = RegistryError::InUse {
            module: id("b"),
            dependents: vec![id("a")],
        };
        assert!(in_use.to_string().contains("wymagany przez: a"));
        assert_eq!(
            registry_event_kind(EVENT_HEALTH).as_str(),
            "registry.module.health"
        );
    }
}
