//! Dostawcy API z kont `accounts-hub` → rejestracja w Routerze. Na dostawcę katalogu przypada
//! jedno konto (najstarsze używalne — trasa `<dostawca>.api` w rejestrze zgodności); model domyślny
//! z wykrytych przy teście konta albo z Models API przy pierwszym użyciu. Zmiana kont (nowy klucz,
//! usunięcie, test) jest widoczna bez restartu: przy każdym wyborze trasy porównywany jest odcisk.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use accounts_hub_contract::{
    Account, AccountsHub, ProviderCatalogEntry, ProviderKind, best_chat_model, model_use,
};
use accounts_hub_impl::AccountsHubService;
use providers_api_impl::{AccountProfile, CatalogEntry, build_provider};
use providers_contract::{ModelKind, ModelProvider, ProviderId};
use router_contract::{RouteKind, TaskClass};
use router_impl::{ProviderQueues, RouterCore};

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
    /// Kolejki modeli konta (klasa → modele w kolejności prób).
    pub queues: BTreeMap<TaskClass, Vec<String>>,
}

impl Registered {
    /// Pierwszy model dla klasy zadań: z kolejki, a bez niej — domyślny.
    pub fn first_model(&self, class: TaskClass) -> Option<String> {
        self.queues
            .get(&class)
            .and_then(|q| q.first().cloned())
            .or_else(|| self.model.clone())
    }
}

/// Klasa zadań huba → klasa Routera (te same nazwy; osobne typy dwóch kontraktów).
fn route_class(class: accounts_hub_contract::TaskClass) -> TaskClass {
    use accounts_hub_contract::TaskClass as Hub;
    match class {
        Hub::VoiceFast => TaskClass::VoiceFast,
        Hub::Conversation => TaskClass::Conversation,
        Hub::Code => TaskClass::Code,
        Hub::Planning => TaskClass::Planning,
        Hub::GuiVision => TaskClass::GuiVision,
        Hub::Summarize => TaskClass::Summarize,
        Hub::Embeddings => TaskClass::Embeddings,
    }
}

fn queues_of(account: &Account) -> BTreeMap<TaskClass, Vec<String>> {
    account
        .queues
        .iter()
        .filter(|(_, models)| !models.is_empty())
        .map(|(class, models)| {
            (
                route_class(*class),
                models.iter().map(|m| m.as_str().to_owned()).collect(),
            )
        })
        .collect()
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

    /// Model domyślny konta: najlepszy model **rozmowy** z listy dostawcy. Lista miesza rozmowę
    /// z syntezą mowy, transkrypcją i obrazami — pierwszy z brzegu bywał `tts-1` i każda
    /// wiadomość kończyła się błędem dostawcy (zgłoszenie właściciela, 2026-10-09).
    fn model_of(&self, account: &Account) -> Option<String> {
        best_chat_model(account.models.iter().map(|m| m.id.as_str()))
            .map(str::to_owned)
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
                Ok(models) => {
                    let chat: Vec<_> = models
                        .into_iter()
                        .filter(|m| {
                            m.capabilities.as_ref().map_or_else(
                                || model_use(&m.id).is_chat(),
                                |c| c.kinds.is_empty() || c.kinds.contains(&ModelKind::Chat),
                            )
                        })
                        .collect();
                    let best =
                        best_chat_model(chat.iter().map(|m| m.id.as_str())).map(str::to_owned);
                    chat.into_iter()
                        .find(|m| best.as_deref().is_none_or(|id| id == m.id))
                }
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
                    "{}|{}|{:?}|{:?}|{:?}",
                    a.provider,
                    a.id,
                    a.base_url,
                    self.model_of(a),
                    a.queues
                )
            })
            .collect();
        if self.lock().fingerprint == fingerprint {
            return;
        }
        let mut registered = BTreeMap::new();
        let mut queues = ProviderQueues::new();
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
                    queues: queues_of(account),
                },
            );
            let own = queues_of(account);
            if !own.is_empty() {
                queues.insert(ProviderId::new(provider_id.as_str()), own);
            }
        }
        for core in &self.cores {
            core.set_provider_queues(queues.clone());
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
