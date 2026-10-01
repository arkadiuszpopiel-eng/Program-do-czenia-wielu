//! „Mózg" = Router (`router-impl`, PLAN §5.4): dostawcy API z kont `accounts-hub`
//! (`providers-api-impl`) i model lokalny (`providers-local-impl`) jako kandydaci; tabela tras
//! automatyczna albo z konfiguracji `[router]`, budżet z `cost-meter`, zgodność z `compliance`.
//! Trzy rdzenie Routera (osobne obwody): hybrydowy (API + lokalny jako zapas/offline), chmurowy
//! (tylko API) i lokalny (tylko model lokalny — profil „Lokalnie" i sesje `local_only`).
//! Stan „brak mózgu" tylko wtedy, gdy Router nie ma żadnego kandydata.

mod accounts;
pub(crate) mod config;
pub(crate) mod local;

use std::collections::BTreeMap;
use std::sync::Arc;

use accounts_hub_impl::AccountsHubService;
use async_trait::async_trait;
use compliance_contract::SessionTag;
use core_bus_contract::{Event, EventBus, Level};
use providers_api_impl::CatalogEntry;
use providers_contract::{HealthState, ModelProvider};
use router_contract::{
    Candidate, Constraints, RejectReason, RouteDecision, RouteError, RouteKind, RouterEvent,
    TaskClass, event_kind,
};
use router_impl::{RoutedProvider, RouterCore};
use sessions_contract::PrivacyTag;

use crate::dto::ModelProfile;
use crate::ports::{BrainChoice, BrainError, BrainPort, BrainRequest, BrainTarget, RouteNote};
use accounts::AccountRoutes;

/// Komunikat „brak mózgu" (PLAN §14.5: bez kluczy startuje profil lokalny).
pub const NO_BRAIN: &str = "Brak mózgu: dodaj klucz API w Ustawieniach → Modele i dostawcy \
     albo pobierz model lokalny.";
/// Profil lokalny bez pobranego modelu (są klucze API).
pub const NO_LOCAL: &str = "Profil lokalny: pobierz model lokalny (Ustawienia → Modele i dostawcy \
     → Lokalne) albo przełącz profil na Hybrydę.";

/// Identyfikator trasy lokalnej (`local:<model>`).
pub const LOCAL_PROVIDER: &str = "local";

/// Rdzenie Routera.
#[derive(Clone)]
pub(crate) struct Routers {
    /// API + lokalny (domyślny profil „Hybryda").
    pub hybrid: Arc<RouterCore>,
    /// Tylko API (profil „Chmura").
    pub cloud: Arc<RouterCore>,
    /// Tylko lokalny (profil „Lokalnie", sesje `local_only`).
    pub local: Arc<RouterCore>,
}

impl Routers {
    /// Rejestruje dostawcę w rdzeniach właściwych dla rodzaju trasy.
    pub fn register(&self, provider: Arc<dyn ModelProvider>, kind: RouteKind) {
        self.hybrid.register(provider.clone(), kind);
        match kind {
            RouteKind::Api => self.cloud.register(provider, kind),
            RouteKind::Local => self.local.register(provider, kind),
        }
    }

    /// Przekazuje zdarzenia `router.*` rdzeni chmurowego i lokalnego na magistralę
    /// (hybrydowy obsługuje `RouterModule` z rejestru).
    pub fn forward_secondary(&self, bus: &Arc<dyn EventBus>) {
        for core in [&self.cloud, &self.local] {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RouterEvent>();
            core.set_event_sink(Some(tx));
            let bus = bus.clone();
            tokio::spawn(async move {
                while let Some(ev) = rx.recv().await {
                    let payload = serde_json::to_value(&ev).unwrap_or_default();
                    let level = match ev {
                        RouterEvent::Decision { .. } | RouterEvent::BreakerClosed { .. } => {
                            Level::Debug
                        }
                        _ => Level::Warn,
                    };
                    // Zdarzenia diagnostyczne — błąd magistrali nie wstrzymuje Routera.
                    let _ = bus
                        .publish(Event::new(event_kind(ev.name()), level, payload))
                        .await;
                }
            });
        }
    }
}

/// Router nie wystartował (błąd budowy modułu) — każda tura kończy się czytelnym błędem.
pub struct RouterUnavailable;

#[async_trait]
impl BrainPort for RouterUnavailable {
    async fn choose(&self, _request: &BrainRequest) -> Result<BrainChoice, BrainError> {
        Err(BrainError::Provider(
            "Router niedostępny (błąd startu modułu router — szczegóły w logach).".into(),
        ))
    }

    fn keys_configured(&self) -> bool {
        false
    }
}

/// Router jako `BrainPort`.
pub struct RouterBrain {
    routers: Routers,
    accounts: AccountRoutes,
}

impl RouterBrain {
    pub(crate) fn new(
        routers: Routers,
        hub: Arc<AccountsHubService>,
        catalog: Arc<BTreeMap<String, CatalogEntry>>,
    ) -> Self {
        let accounts = AccountRoutes::new(
            hub,
            catalog,
            vec![routers.hybrid.clone(), routers.cloud.clone()],
        );
        Self { routers, accounts }
    }

    fn api_ready(&self) -> bool {
        self.routers.cloud.all_registered().iter().any(|(_, r)| {
            r.kind == RouteKind::Api && r.provider.health().state != HealthState::Unconfigured
        })
    }

    fn no_route(&self, error: &RouteError, local_only: bool) -> BrainError {
        let RouteError::NoRoute { rejected, .. } = error;
        let missing = rejected
            .iter()
            .all(|(_, r)| matches!(r, RejectReason::NotRegistered | RejectReason::Unconfigured));
        if missing
            && !local_only
            && let Some(failure) = self.accounts.detection_failure()
        {
            return BrainError::Provider(format!(
                "Dostawca jest skonfigurowany, ale niedostępny ({failure}). Sprawdź połączenie \
                 albo przetestuj konto w Ustawieniach → Modele i dostawcy."
            ));
        }
        if missing {
            let text = if local_only && self.api_ready() {
                NO_LOCAL
            } else {
                NO_BRAIN
            };
            return BrainError::NoKeys(text.into());
        }
        let reasons = rejected
            .iter()
            .map(|(c, r)| format!("{c} — {r}"))
            .collect::<Vec<_>>()
            .join("; ");
        if rejected
            .iter()
            .all(|(_, r)| matches!(r, RejectReason::Budget { .. }))
        {
            return BrainError::Budget(format!(
                "Limit kosztów osiągnięty ({reasons}) — zmień limit w Ustawieniach → Koszty."
            ));
        }
        BrainError::Provider(format!("Brak dozwolonej trasy dla tej sesji: {reasons}."))
    }
}

fn note(decision: &RouteDecision) -> RouteNote {
    RouteNote {
        chosen: decision.chosen.qualified(),
        fallbacks: decision
            .fallbacks
            .iter()
            .map(Candidate::qualified)
            .collect(),
        rejected: decision
            .rejected
            .iter()
            .map(|(c, r)| format!("{c} — {r}"))
            .collect(),
    }
}

#[async_trait]
impl BrainPort for RouterBrain {
    async fn choose(&self, request: &BrainRequest) -> Result<BrainChoice, BrainError> {
        self.accounts.detect_models().await;
        self.accounts.sync();
        let local_only = request.privacy == PrivacyTag::LocalOnly
            || request.profile == Some(ModelProfile::Local);
        let core = if local_only {
            &self.routers.local
        } else if request.profile == Some(ModelProfile::Cloud) {
            &self.routers.cloud
        } else {
            &self.routers.hybrid
        };
        let mut constraints = request
            .chat
            .as_ref()
            .map(|c| Constraints::from_request(TaskClass::Conversation, c))
            .unwrap_or_default();
        constraints.session = match request.privacy {
            PrivacyTag::Normal => SessionTag::Standard,
            PrivacyTag::Private | PrivacyTag::LocalOnly => SessionTag::Private,
        };
        // Konto przypisane agentce jest próbowane pierwsze (przypięcie), reszta jako zapas.
        constraints.pinned = (!local_only)
            .then(|| self.accounts.assigned_to(&request.agent))
            .flatten()
            .and_then(|(provider, r)| r.model.map(|m| Candidate::new(provider, m)));
        let decision = core
            .decide(TaskClass::Conversation, &constraints, request.chat.as_ref())
            .map_err(|e| self.no_route(&e, local_only))?;
        let context_window = core.registered(&decision.chosen.provider).and_then(|r| {
            r.provider
                .capabilities()
                .models
                .get(&decision.chosen.model)
                .and_then(|m| m.context_window)
                .map(u64::from)
        });
        Ok(BrainChoice {
            provider: Arc::new(RoutedProvider::new(core.clone(), TaskClass::Conversation)),
            provider_id: router_contract::ROUTER_PROVIDER_ID.into(),
            provider_name: "Router".into(),
            account: None,
            model: constraints.pinned.as_ref().map_or_else(
                || router_contract::AUTO_MODEL.to_owned(),
                Candidate::qualified,
            ),
            context_window,
            routed: true,
            route: Some(note(&decision)),
        })
    }

    fn keys_configured(&self) -> bool {
        self.accounts.sync();
        self.api_ready()
    }

    fn target(&self, model: &str) -> Option<BrainTarget> {
        let c = Candidate::parse(model)?;
        let provider = c.provider.as_str().to_owned();
        if provider == LOCAL_PROVIDER {
            return Some(BrainTarget {
                provider_id: provider,
                provider_name: "Model lokalny".into(),
                account: None,
                model: c.model,
                local: true,
            });
        }
        let registered = self.accounts.registered(&provider);
        Some(BrainTarget {
            provider_name: registered
                .as_ref()
                .map_or_else(|| provider.clone(), |r| r.name.clone()),
            account: registered.map(|r| r.account),
            provider_id: provider,
            model: c.model,
            local: false,
        })
    }
}
