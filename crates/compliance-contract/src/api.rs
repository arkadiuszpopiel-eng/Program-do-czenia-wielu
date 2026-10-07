//! Trait `Compliance`, błędy, pochodzenie zmian, zegar dat i nazwy zdarzeń.

use async_trait::async_trait;
use chrono::NaiveDate;
use core_bus_contract::EventKind;
use serde::{Deserialize, Serialize};

use crate::decision::Decision;
use crate::deny::{DenyLists, KernelAuthority};
use crate::registry::{RegistryError, RegistryRoute};
use crate::status::{EffectiveStatus, RouteId};
use crate::table::RouteView;
use crate::tags::{RouteTags, SessionTag};

/// Zdarzenie: użytkownik/Broker włączył trasę.
pub const EVENT_ROUTE_ENABLED: &str = "compliance.route.enabled";
/// Zdarzenie: trasa wyłączona.
pub const EVENT_ROUTE_DISABLED: &str = "compliance.route.disabled";
/// Zdarzenie: trasa zdegradowana do „szarej” z powodu nieświeżego rejestru.
pub const EVENT_ROUTE_STALE: &str = "compliance.route.stale";
/// Zdarzenie: Broker podmienił deny-listy.
pub const EVENT_DENYLIST_UPDATED: &str = "compliance.denylist.updated";

/// Rodzaj zdarzenia jako `EventKind`.
pub fn event_kind(name: &str) -> EventKind {
    EventKind::Custom(name.to_owned())
}

/// Kto zmienia stan rejestru.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", content = "id", rename_all = "snake_case")]
pub enum ChangeOrigin {
    /// Użytkownik (karta zgodności w UI).
    User,
    /// Broker (polityki Jądra).
    Broker,
    /// Agentka — nie zmienia rejestru.
    Agent(String),
    /// Ulepszacz — nie zmienia rejestru (PLAN §12.4).
    Improver,
    /// Inny moduł — nie zmienia rejestru.
    Module(String),
}

impl ChangeOrigin {
    /// Wyłącznik trasy przestawia tylko użytkownik albo Broker.
    pub fn may_toggle_routes(&self) -> bool {
        matches!(self, ChangeOrigin::User | ChangeOrigin::Broker)
    }
}

/// Błędy modułu zgodności.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ComplianceError {
    /// Nieznana trasa.
    #[error("nieznana trasa `{0}`")]
    UnknownRoute(RouteId),
    /// Próba włączenia trasy zabronionej.
    #[error("trasa `{0}` jest zabroniona i nie może zostać włączona")]
    ForbiddenRoute(RouteId),
    /// Inicjator nie może zmieniać rejestru.
    #[error("brak uprawnień do zmiany rejestru zgodności: {0:?}")]
    NotPermitted(ChangeOrigin),
    /// Niepoprawna deny-lista.
    #[error("niepoprawna deny-lista: {0}")]
    InvalidDenyList(String),
    /// Błąd rejestru.
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

/// Źródło daty „dziś” (lokalnej); atrapa steruje nim w testach.
pub trait Today: Send + Sync {
    /// Bieżąca data.
    fn today(&self) -> NaiveDate;
}

/// Kontrakt modułu zgodności (v0).
#[async_trait]
pub trait Compliance: Send + Sync {
    /// Wpis rejestru (źródła, cytaty — do karty zgodności). `None` dla tras API z katalogu.
    fn route(&self, id: &RouteId) -> Option<RegistryRoute>;

    /// Widok trasy (status efektywny, wyłącznik, tagi).
    fn view(&self, id: &RouteId) -> Option<RouteView>;

    /// Widoki wszystkich tras.
    fn views(&self) -> Vec<RouteView>;

    /// Status efektywny (uwzględnia świeżość).
    fn effective_status(&self, id: &RouteId) -> Option<EffectiveStatus> {
        self.view(id).map(|v| v.effective)
    }

    /// Tagi prywatności/jurysdykcji trasy.
    fn tags(&self, id: &RouteId) -> Option<RouteTags> {
        self.view(id).map(|v| v.tags)
    }

    /// Czy trasa może obsłużyć sesję o danym tagu (z powodem).
    fn route_allowed(&self, id: &RouteId, session: SessionTag) -> Decision;

    /// Przestawia wyłącznik trasy (tylko użytkownik/Broker; zabronionej nie da się włączyć).
    async fn set_enabled(
        &self,
        id: &RouteId,
        on: bool,
        origin: ChangeOrigin,
    ) -> Result<RouteView, ComplianceError>;

    /// Bieżące deny-listy (kopia danych).
    fn deny_lists(&self) -> DenyLists;

    /// Czy ścieżka jest na deny-liście poświadczeń (po normalizacji ścieżek Windows).
    fn is_denied_path(&self, path: &str) -> bool;

    /// Czy domena (lub URL) jest na deny-liście webowych UI dostawców.
    fn is_denied_domain(&self, domain: &str) -> bool;

    /// Podmienia deny-listy — wyłącznie z dowodem uprawnień Brokera.
    async fn replace_deny_lists(
        &self,
        authority: &KernelAuthority,
        lists: DenyLists,
    ) -> Result<(), ComplianceError>;
}
