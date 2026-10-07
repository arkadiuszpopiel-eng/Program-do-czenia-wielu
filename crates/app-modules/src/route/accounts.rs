//! Dostawcy API z kont `accounts-hub` → rejestracja w Routerze. Na dostawcę katalogu przypada
//! jedno konto (najstarsze używalne — trasa `<dostawca>.api` w rejestrze zgodności); model domyślny
//! z wykrytych przy teście konta albo z Models API przy pierwszym użyciu. Zmiana kont (nowy klucz,
//! usunięcie, test) jest widoczna bez restartu: przy każdym wyborze trasy porównywany jest odcisk.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use accounts_hub_contract::{Account, AccountsHub, ProviderCatalogEntry, ProviderKind};
use accounts_hub_impl::AccountsHubService;
use providers_api_impl::{AccountProfile, CatalogEntry, build_provider};
use providers_contract::{ModelKind, ModelProvider, ProviderId};
use router_contract::RouteKind;
use router_impl::RouterCore;

use crate::secrets::HubKeySource;

/// Konto zarejestrowane jako trasa API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Registered {
    /// Konto.
    pub account: String,
    /// Nazwa dostawcy do UI.
    pub name: String,
    /// Model domyślny.
    pub model: Option<String>,
}

#[derive(Default)]
struct State {
    fingerprint: Vec<String>,
    registered: BTreeMap<String, Registered>,
    /// Modele wykryte przez Models API dla kont bez modeli z testu (konto → model).
    detected: HashMap<String, String>,
    /// Ostatni błąd wykrywania modeli (dostawca → opis) — zamiast „brak mózgu".
    failed: HashMap<String, String>,
}

/// Synchronizacja kont z Routerami.
pub(crate) struct AccountRoutes {
    hub: Arc<AccountsHubService>,
    catalog: Arc<BTreeMap<String, CatalogEntry>>,
    /// Routery z trasami API (hybrydowy, chmurowy).
    cores: Vec<Arc<RouterCore>>,
    state: Mutex<State>,
}

impl AccountRoutes {
    pub fn new(
        hub: Arc<AccountsHubService>,
        catalog: Arc<BTreeMap<String, CatalogEntry>>,
        cores: Vec<Arc<RouterCore>>,
    ) -> Self {
        Self {
            hub,
            catalog,
            cores,
            state: Mutex::new(State::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn entry(&self, account: &Account) -> Option<ProviderCatalogEntry> {
        self.hub.provider(&account.provider)
    }

    /// Konta czatu, które mogą odpowiadać (najstarsze najpierw).
    pub fn eligible(&self) -> Vec<Account> {
        let mut accounts: Vec<Account> = self
            .hub
            .accounts()
            .into_iter()
            .filter(|a| a.state.is_usable())
            .filter(|a| self.catalog.contains_key(a.provider.as_str()))
            .filter(|a| {
                self.entry(a)
                    .is_some_and(|e| matches!(e.kind, ProviderKind::Chat | ProviderKind::Multi))
            })
            .collect();
        accounts.sort_by_key(|a| a.created_at);
        accounts
    }

    /// Konto przypisane agentce (pierwsze używalne), jeśli jest.
    pub fn assigned_to(&self, agent: &str) -> Option<(String, Registered)> {
        let state = self.lock();
        self.eligible()
            .into_iter()
            .find(|a| a.assignments.agents.iter().any(|x| x == agent))
            .and_then(|a| {
                state
                    .registered
                    .get(a.provider.as_str())
                    .filter(|r| r.account == a.id.as_str())
                    .map(|r| (a.provider.to_string(), r.clone()))
            })
    }

    /// Dostawca z kluczem, którego modelu nie udało się wykryć: „nazwa: opis błędu".
    pub fn detection_failure(&self) -> Option<String> {
        let state = self.lock();
        state.failed.iter().next().map(|(provider, error)| {
            let name = state
                .registered
                .get(provider)
                .map_or_else(|| provider.clone(), |r| r.name.clone());
            format!("{name}: {error}")
        })
    }

    /// Zarejestrowana trasa API dostawcy.
    pub fn registered(&self, provider: &str) -> Option<Registered> {
        self.lock().registered.get(provider).cloned()
    }

    fn model_of(&self, account: &Account) -> Option<String> {
        account
            .models
            .first()
            .map(|m| m.id.as_str().to_owned())
            .or_else(|| self.lock().detected.get(account.id.as_str()).cloned())
    }

    fn build(&self, account: &Account, model: Option<String>) -> Option<Arc<dyn ModelProvider>> {
        let entry = self.catalog.get(account.provider.as_str())?;
        let hub: Arc<dyn AccountsHub> = self.hub.clone();
        let mut profile = AccountProfile::new(Arc::new(HubKeySource::new(hub, account.id.clone())));
        profile.base_url.clone_from(&account.base_url);
        profile.default_model = model;
        match build_provider(entry, profile) {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!(dostawca = %account.provider, error = %e, "adapter dostawcy niedostępny");
                None
            }
        }
    }

    /// Wykrywa model kont bez modeli z testu (Models API; raz na konto).
    pub async fn detect_models(&self) {
        let missing: Vec<Account> = self
            .eligible()
            .into_iter()
            .filter(|a| self.model_of(a).is_none())
            .collect();
        for account in missing {
            let Some(provider) = self.build(&account, None) else {
                continue;
            };
            let provider_id = account.provider.as_str().to_owned();
            let found = match provider.list_models().await {
                Ok(models) => models.into_iter().find(|m| {
                    m.capabilities
                        .as_ref()
                        .is_none_or(|c| c.kinds.is_empty() || c.kinds.contains(&ModelKind::Chat))
                }),
                Err(e) => {
                    tracing::warn!(konto = %account.id, error = %e, "wykrycie modeli nie powiodło się");
                    self.lock()
                        .failed
                        .insert(provider_id.clone(), format!("wykrycie modeli: {e}"));
                    continue;
                }
            };
            let mut state = self.lock();
            match found {
                Some(model) => {
                    state.failed.remove(&provider_id);
                    state
                        .detected
                        .insert(account.id.as_str().to_owned(), model.id);
                }
                None => {
                    state.failed.insert(
                        provider_id,
                        "dostawca nie zwrócił żadnego modelu czatu".into(),
                    );
                }
            }
        }
    }

    /// Rejestruje/wyrejestrowuje trasy API, gdy konta się zmieniły.
    pub fn sync(&self) {
        let mut chosen: BTreeMap<String, Account> = BTreeMap::new();
        for account in self.eligible() {
            chosen
                .entry(account.provider.as_str().to_owned())
                .or_insert(account);
        }
        let fingerprint: Vec<String> = chosen
            .values()
            .map(|a| {
                format!(
                    "{}|{}|{:?}|{:?}",
                    a.provider,
                    a.id,
                    a.base_url,
                    self.model_of(a)
                )
            })
            .collect();
        if self.lock().fingerprint == fingerprint {
            return;
        }
        let mut registered = BTreeMap::new();
        for (provider_id, account) in &chosen {
            let model = self.model_of(account);
            let Some(provider) = self.build(account, model.clone()) else {
                continue;
            };
            for core in &self.cores {
                core.register(provider.clone(), RouteKind::Api);
            }
            let name = self
                .entry(account)
                .map_or_else(|| provider_id.clone(), |e| e.display_name);
            registered.insert(
                provider_id.clone(),
                Registered {
                    account: account.id.as_str().to_owned(),
                    name,
                    model,
                },
            );
        }
        let mut state = self.lock();
        state
            .failed
            .retain(|provider, _| chosen.contains_key(provider));
        for gone in state
            .registered
            .keys()
            .filter(|k| !registered.contains_key(*k))
        {
            for core in &self.cores {
                core.unregister(&ProviderId::new(gone.as_str()));
            }
        }
        state.registered = registered;
        state.fingerprint = fingerprint;
    }
}
