//! „Mózg" = Router (`router-impl`, PLAN §5.4): dostawcy API z kont `accounts-hub`
//! (`providers-api-impl`) i model lokalny (`providers-local-impl`) jako kandydaci; tabela tras
//! automatyczna albo z konfiguracji `[router]`, budżet z `cost-meter`, zgodność z `compliance`.
//! Trzy rdzenie Routera (osobne obwody): hybrydowy (API + lokalny jako zapas/offline), chmurowy
//! (tylko API) i lokalny (tylko model lokalny — profil „Lokalnie" i sesje `local_only`).
//! Stan „brak mózgu" tylko wtedy, gdy Router nie ma żadnego kandydata.

mod accounts;
pub mod config;
pub mod engine_settings;
pub mod global_queue;
pub mod local;
pub mod routing_settings;

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

use accounts::AccountRoutes;
use app_api::dto::{ModelProfile, ModelRouting, ModelRoutingSettings, RoutingPreview};
use app_api::ports::{BrainChoice, BrainError, BrainPort, BrainRequest, BrainTarget, RouteNote};
use routing_settings::PolicyFile;

/// Komunikat „brak mózgu" (PLAN §14.5: bez kluczy startuje profil lokalny).
pub const NO_BRAIN: &str = "Brak mózgu: dodaj klucz API w Ustawieniach → Konta i klucze API \
     albo pobierz model lokalny.";
/// Profil lokalny bez pobranego modelu (są klucze API).
pub const NO_LOCAL: &str = "Profil lokalny: pobierz model lokalny (Ustawienia → Modele lokalne i silniki) albo przełącz profil na Hybrydę.";

/// Identyfikator trasy lokalnej (`local:<model>`).
pub const LOCAL_PROVIDER: &str = "local";

/// Rdzenie Routera.
#[derive(Clone)]
pub struct Routers {
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
    /// Ustawienia doboru według trudności (`model-routing.json`).
    routing: PolicyFile,
}

impl RouterBrain {
    /// Router na rdzeniach z kontami hubu i katalogiem dostawców API.
    pub fn new(
        routers: Routers,
        hub: Arc<AccountsHubService>,
        catalog: Arc<BTreeMap<String, CatalogEntry>>,
        shared_queues: std::path::PathBuf,
        routing: std::path::PathBuf,
    ) -> Self {
        let accounts = AccountRoutes::new(
            hub,
            catalog,
            vec![routers.hybrid.clone(), routers.cloud.clone()],
            shared_queues,
        );
        Self {
            routers,
            accounts,
            routing: PolicyFile::new(routing),
        }
    }

    fn cores(&self) -> [&Arc<RouterCore>; 3] {
        [
            &self.routers.hybrid,
            &self.routers.cloud,
            &self.routers.local,
        ]
    }

    /// Nakłada ustawienia doboru na rdzenie, gdy plik się zmienił.
    fn apply_routing(&self) {
        if let Some(policy) = self.routing.changed() {
            for core in self.cores() {
                core.set_difficulty_policy(policy.clone());
            }
        }
    }

    /// Nazwa dostawcy kandydata do interfejsu.
    fn provider_name(&self, c: &Candidate) -> String {
        self.target(&c.qualified())
            .map_or_else(|| c.provider.as_str().to_owned(), |t| t.provider_name)
    }

    fn routing_view(&self) -> ModelRouting {
        self.accounts.sync();
        self.apply_routing();
        let hybrid = &self.routers.hybrid;
        let models = routing_settings::candidates(&[hybrid, &self.routers.local])
            .iter()
            .map(|c| routing_settings::model_of(hybrid, c, self.provider_name(c)))
            .collect();
        ModelRouting {
            settings: routing_settings::settings_dto(&hybrid.difficulty_policy()),
            models,
        }
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
                 albo przetestuj konto w Ustawieniach → Konta i klucze API."
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

/// Klasa zadań Routera z „klasy modelu” roli agentki (`model_policy` roli): programistka idzie
/// kolejką „Kod”, planistka i recenzentka — „Planowanie”; reszta to rozmowa.
pub fn task_class(policy: Option<&str>) -> TaskClass {
    router_contract::class_from_policy(policy.unwrap_or_default())
        .unwrap_or(TaskClass::Conversation)
}

/// Przypięcie trasy: model wskazany w żądaniu ma pierwszeństwo przed kontem agentki.
fn pinned_first(requested: Option<Candidate>, assigned: Option<Candidate>) -> Option<Candidate> {
    requested.or(assigned)
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
        difficulty: decision.difficulty.as_ref().map(|d| d.summary()),
    }
}

#[async_trait]
impl BrainPort for RouterBrain {
    async fn choose(&self, request: &BrainRequest) -> Result<BrainChoice, BrainError> {
        self.accounts.detect_models().await;
        self.accounts.sync();
        self.apply_routing();
        let local_only = request.privacy == PrivacyTag::LocalOnly
            || request.profile == Some(ModelProfile::Local);
        let core = if local_only {
            &self.routers.local
        } else if request.profile == Some(ModelProfile::Cloud) {
            &self.routers.cloud
        } else {
            &self.routers.hybrid
        };
        let class = task_class(request.task.as_deref());
        let mut constraints = request
            .chat
            .as_ref()
            .map(|c| Constraints::from_request(class, c))
            .unwrap_or_default();
        constraints.session = match request.privacy {
            PrivacyTag::Normal => SessionTag::Standard,
            PrivacyTag::Private | PrivacyTag::LocalOnly => SessionTag::Private,
        };
        // Model wybrany przez użytkownika (Arena: `dostawca:model` w żądaniu) jest próbowany
        // pierwszy; bez niego — konto przypisane agentce. Reszta jako zapas; kontrole prywatności,
        // zgodności i budżetu obowiązują także przypięty model.
        let assigned = (!local_only)
            .then(|| self.accounts.assigned_to(&request.agent))
            .flatten()
            .and_then(|(provider, r)| r.first_model(class).map(|m| Candidate::new(provider, m)));
        constraints.pinned = pinned_first(constraints.pinned.take(), assigned);
        let decision = core
            .decide(class, &constraints, request.chat.as_ref())
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
            provider: Arc::new(RoutedProvider::new(core.clone(), class)),
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

    fn bench_account(&self, account: &str, until: chrono::DateTime<chrono::Utc>) -> Option<String> {
        self.accounts.bench(account, until).map(|a| a.label)
    }

    fn backup_for(&self, account: &str) -> Option<String> {
        self.accounts.backup_for(account).map(|a| a.label)
    }

    fn routing(&self) -> Option<ModelRouting> {
        Some(self.routing_view())
    }

    fn set_routing(&self, settings: ModelRoutingSettings) -> Result<ModelRouting, String> {
        let policy = routing_settings::policy_of(&settings)?;
        routing_settings::save(self.routing.path(), &policy)
            .map_err(|e| format!("Nie udało się zapisać ustawień doboru modelu: {e}."))?;
        self.routing.forget();
        Ok(self.routing_view())
    }

    fn routing_preview(&self, text: &str, class: &str) -> Option<RoutingPreview> {
        let class = global_queue::class_of(class)?;
        self.accounts.sync();
        self.apply_routing();
        Some(routing_settings::preview(
            &self.routers.hybrid,
            class,
            text,
            |c| self.provider_name(c),
        ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requested_model_wins_over_agent_account() {
        let arena = Candidate::new("openai", "gpt-6-sol");
        let agent = Candidate::new("anthropic", "claude-opus-5-5");
        assert_eq!(
            pinned_first(Some(arena.clone()), Some(agent.clone())),
            Some(arena)
        );
        assert_eq!(pinned_first(None, Some(agent.clone())), Some(agent));
        assert_eq!(pinned_first(None, None), None);
    }
}
