//! Tabela tras: rejestr + trasy API z katalogu + wyłączniki użytkownika. Czysta logika
//! współdzielona przez `-impl` i `-fake` (żeby atrapa nie rozjechała się z implementacją).

use std::collections::BTreeMap;

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::api::{ChangeOrigin, ComplianceError};
use crate::decision::{Decision, PrivacyPolicy, decide};
use crate::registry::{Registry, RouteMode};
use crate::status::{EffectiveStatus, ProviderApiStatus, RouteId, RouteStatus, effective_status};
use crate::tags::{RouteTags, SessionTag};

/// Dane dostawcy z katalogu (`providers-catalog/*.toml`) przekazywane przy budowie tabeli.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderPolicyInput {
    /// Identyfikator dostawcy.
    pub provider: String,
    /// Tagi z katalogu (łączone z tagami rejestru — suma, ostrożnie).
    pub tags: RouteTags,
    /// Status API z katalogu.
    pub api_status: ProviderApiStatus,
}

/// Skąd pochodzi trasa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RouteOrigin {
    /// Wpis rejestru zgodności (mosty CLI/SDK, trasy API opisane w rejestrze).
    Registry,
    /// Trasa API dostawcy z katalogu (`<provider>.api`).
    Catalog,
}

/// Widok trasy dla Routera i UI (karta zgodności).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RouteView {
    /// Identyfikator.
    pub id: RouteId,
    /// Dostawca.
    pub provider: String,
    /// Tryb.
    pub mode: RouteMode,
    /// Pochodzenie.
    pub origin: RouteOrigin,
    /// Status zadeklarowany.
    pub declared: RouteStatus,
    /// Status efektywny (świeżość, weryfikacja).
    pub effective: EffectiveStatus,
    /// Data weryfikacji (tylko trasy z rejestru).
    pub verified_at: Option<NaiveDate>,
    /// Czy trasa jest włączona (wyłącznik użytkownika albo wartość domyślna).
    pub enabled: bool,
    /// Jawne ustawienie użytkownika/Brokera, jeśli było.
    pub user_override: Option<bool>,
    /// Tagi prywatności/jurysdykcji.
    pub tags: RouteTags,
}

/// Ustawienia tabeli.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TableSettings {
    /// Próg nieświeżości w dniach; `None` = `max_age_days` z rejestru.
    pub max_age_days: Option<u32>,
    /// Polityka sesji prywatnych.
    pub policy: PrivacyPolicy,
}

/// Tabela tras.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteTable {
    registry: Registry,
    catalog: BTreeMap<String, ProviderPolicyInput>,
    overrides: BTreeMap<RouteId, bool>,
    max_age_days: u32,
    policy: PrivacyPolicy,
}

impl RouteTable {
    /// Buduje tabelę z rejestru, wpisów katalogu i ustawień.
    pub fn new(
        registry: Registry,
        catalog: Vec<ProviderPolicyInput>,
        settings: TableSettings,
    ) -> Self {
        let max_age_days = settings
            .max_age_days
            .filter(|d| *d > 0)
            .unwrap_or(registry.max_age_days);
        Self {
            catalog: catalog
                .into_iter()
                .map(|c| (c.provider.clone(), c))
                .collect(),
            registry,
            overrides: BTreeMap::new(),
            max_age_days,
            policy: settings.policy,
        }
    }

    /// Rejestr źródłowy.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Obowiązujący próg nieświeżości (dni).
    pub fn max_age_days(&self) -> u32 {
        self.max_age_days
    }

    /// Jawne wyłączniki.
    pub fn overrides(&self) -> &BTreeMap<RouteId, bool> {
        &self.overrides
    }

    /// Wszystkie identyfikatory tras (rejestr, potem trasy API z katalogu).
    pub fn ids(&self) -> Vec<RouteId> {
        let registry = self.registry.routes.iter().map(|r| r.id.clone());
        let api = self.catalog.keys().filter_map(|p| RouteId::api(p));
        registry.chain(api).collect()
    }

    /// Tagi dostawcy: rejestr ∪ katalog; brak danych = puste (traktowane jak nieznane).
    pub fn provider_tags(&self, provider: &str) -> RouteTags {
        let from_registry = self
            .registry
            .providers
            .iter()
            .find(|p| p.id == provider)
            .map(|p| RouteTags {
                privacy: p.privacy_tags.clone(),
                jurisdiction: p.jurisdiction.clone(),
            })
            .unwrap_or_default();
        match self.catalog.get(provider) {
            Some(c) => from_registry.union(&c.tags),
            None => from_registry,
        }
    }

    /// Widok trasy na dany dzień.
    pub fn view(&self, id: &RouteId, today: NaiveDate) -> Option<RouteView> {
        let user_override = self.overrides.get(id).copied();
        if let Some(r) = self.registry.route(id) {
            let effective =
                effective_status(r.status, Some(r.verified_at), today, self.max_age_days);
            let default_on = r.enabled_by_default && !effective.stale;
            return Some(RouteView {
                id: id.clone(),
                provider: r.provider.clone(),
                mode: r.mode,
                origin: RouteOrigin::Registry,
                declared: r.status,
                effective,
                verified_at: Some(r.verified_at),
                enabled: enabled(effective.status, user_override, default_on),
                user_override,
                tags: RouteTags {
                    privacy: r.privacy_tags.clone(),
                    jurisdiction: Default::default(),
                }
                .union(&self.provider_tags(&r.provider)),
            });
        }
        let provider = id.api_provider()?;
        let entry = self.catalog.get(provider)?;
        let declared = entry.api_status.route_status();
        let effective = EffectiveStatus {
            status: declared,
            stale: false,
            unverified: entry.api_status == ProviderApiStatus::Unverified,
        };
        Some(RouteView {
            id: id.clone(),
            provider: provider.to_owned(),
            mode: RouteMode::Api,
            origin: RouteOrigin::Catalog,
            declared,
            effective,
            verified_at: None,
            enabled: enabled(declared, user_override, true),
            user_override,
            tags: self.provider_tags(provider),
        })
    }

    /// Widoki wszystkich tras.
    pub fn views(&self, today: NaiveDate) -> Vec<RouteView> {
        self.ids()
            .iter()
            .filter_map(|id| self.view(id, today))
            .collect()
    }

    /// Trasy zdegradowane z powodu nieświeżości.
    pub fn stale_routes(&self, today: NaiveDate) -> Vec<RouteId> {
        self.views(today)
            .into_iter()
            .filter(|v| v.effective.stale)
            .map(|v| v.id)
            .collect()
    }

    /// Decyzja dla sesji.
    pub fn decide(&self, id: &RouteId, session: SessionTag, today: NaiveDate) -> Decision {
        decide(self.view(id, today).as_ref(), session, &self.policy)
    }

    /// Sprawdza, czy `origin` może przestawić wyłącznik (bez zmiany stanu).
    pub fn check_toggle(
        &self,
        id: &RouteId,
        on: bool,
        origin: &ChangeOrigin,
        today: NaiveDate,
    ) -> Result<RouteView, ComplianceError> {
        if !origin.may_toggle_routes() {
            return Err(ComplianceError::NotPermitted(origin.clone()));
        }
        let view = self
            .view(id, today)
            .ok_or_else(|| ComplianceError::UnknownRoute(id.clone()))?;
        if on && view.effective.status == RouteStatus::Forbidden {
            return Err(ComplianceError::ForbiddenRoute(id.clone()));
        }
        Ok(view)
    }

    /// Zmienia zadeklarowany status trasy z rejestru (atrapy i narzędzia weryfikacji; w produkcji
    /// rejestr zmienia się wyłącznie commitem w repo). `false`, gdy trasy nie ma w rejestrze.
    pub fn set_declared_status(&mut self, id: &RouteId, status: RouteStatus) -> bool {
        match self.registry.routes.iter_mut().find(|r| &r.id == id) {
            Some(route) => {
                route.status = status;
                true
            }
            None => false,
        }
    }

    /// Przestawia wyłącznik po `check_toggle`; zwraca widok przed i po zmianie.
    pub fn toggle(
        &mut self,
        id: &RouteId,
        on: bool,
        origin: &ChangeOrigin,
        today: NaiveDate,
    ) -> Result<(RouteView, RouteView), ComplianceError> {
        let before = self.check_toggle(id, on, origin, today)?;
        self.overrides.insert(id.clone(), on);
        let after = self
            .view(id, today)
            .ok_or_else(|| ComplianceError::UnknownRoute(id.clone()))?;
        Ok((before, after))
    }
}

fn enabled(status: RouteStatus, user_override: Option<bool>, default_on: bool) -> bool {
    status != RouteStatus::Forbidden && user_override.unwrap_or(default_on)
}
