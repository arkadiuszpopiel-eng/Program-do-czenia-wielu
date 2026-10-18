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
use providers_api_impl::{CatalogEntry, build_provider};
use providers_contract::{ModelKind, ModelProvider, ProviderId};
use router_contract::{Candidate, RouteKind, TaskClass};
use router_impl::{GlobalQueues, ProviderQueues, RouterCore};

use super::global_queue::{QueueFile, SharedQueues, class_of};

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

/// Wspólna kolejka → kandydaci Routera. Pozycja konta, które nie jest trasą swojego dostawcy
/// (wyłączone, bez klucza, drugie konto tego samego dostawcy), jest pomijana — Router zna tylko
/// jedną trasę na dostawcę.
fn global_candidates(shared: &SharedQueues, chosen: &BTreeMap<String, Account>) -> GlobalQueues {
    shared
        .iter()
        .filter_map(|(name, entries)| {
            let class = class_of(name)?;
            let list: Vec<Candidate> = entries
                .iter()
                .filter_map(|entry| {
                    let (provider, _) = chosen
                        .iter()
                        .find(|(_, account)| account.id.as_str() == entry.account_id)?;
                    Some(Candidate::new(provider.as_str(), entry.model.clone()))
                })
                .collect();
            (!list.is_empty()).then_some((class, list))
        })
        .collect()
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
    /// Konta odstawione po limicie zapytań (konto → do kiedy): w tym czasie pierwszeństwo ma
    /// inne konto tego samego dostawcy (plan v6, zadanie C2.15).
    benched: HashMap<String, chrono::DateTime<chrono::Utc>>,
}

/// Synchronizacja kont z Routerami.
pub(crate) struct AccountRoutes {
    hub: Arc<AccountsHubService>,
    catalog: Arc<BTreeMap<String, CatalogEntry>>,
    /// Routery z trasami API (hybrydowy, chmurowy).
    cores: Vec<Arc<RouterCore>>,
    /// Wspólna kolejka wszystkich kont (`model-queues.json`).
    shared: QueueFile,
    state: Mutex<State>,
}

impl AccountRoutes {
    pub fn new(
        hub: Arc<AccountsHubService>,
        catalog: Arc<BTreeMap<String, CatalogEntry>>,
        cores: Vec<Arc<RouterCore>>,
        shared: std::path::PathBuf,
    ) -> Self {
        Self {
            hub,
            catalog,
            cores,
            shared: QueueFile::new(shared),
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
        // Konto odstawione po limicie zapytań idzie na koniec (zostaje używalne, gdy nie ma
        // innego); po upływie czasu wraca na swoje miejsce.
        let now = chrono::Utc::now();
        let benched: Vec<String> = {
            let mut state = self.lock();
            state.benched.retain(|_, until| *until > now);
            state.benched.keys().cloned().collect()
        };
        accounts.sort_by_key(|a| (benched.iter().any(|b| b == a.id.as_str()), a.created_at));
        accounts
    }

    /// Inne używalne konto tego samego dostawcy (konto zapasowe), jeśli jest.
    pub fn backup_for(&self, account: &str) -> Option<Account> {
        let all = self.eligible();
        let provider = self
            .hub
            .accounts()
            .into_iter()
            .find(|a| a.id.as_str() == account)?
            .provider;
        all.into_iter()
            .find(|a| a.provider == provider && a.id.as_str() != account)
    }

    /// Odstawia konto do `until`; zwraca konto zapasowe, które przejmie zapytania (bez
    /// zapasowego nic nie jest odstawiane — jedyne konto ma dalej próbować).
    pub fn bench(&self, account: &str, until: chrono::DateTime<chrono::Utc>) -> Option<Account> {
        let backup = self.backup_for(account)?;
        self.lock().benched.insert(account.to_owned(), until);
        self.sync();
        Some(backup)
    }

    /// Konto przypisane agentce (pierwsze używalne), jeśli jest.
    pub fn assigned_to(&self, agent: &str) -> Option<(String, Registered)> {
        // Lista kont przed zamkiem stanu: `eligible` sama z niego korzysta.
        let eligible = self.eligible();
        let state = self.lock();
        eligible
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
        let key = Arc::new(HubKeySource::new(hub, account.id.clone()));
        let base_url = account.base_url.as_deref();
        let mut profile = crate::probe::account_profile(key, base_url, &account.enterprise);
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
        let shared = self.shared.current();
        let mut chosen: BTreeMap<String, Account> = BTreeMap::new();
        for account in self.eligible() {
            chosen
                .entry(account.provider.as_str().to_owned())
                .or_insert(account);
        }
        let fingerprint: Vec<String> = chosen
            .values()
            .map(|a| {
                // Ustawienia firmowe w całości (nie `Debug`, który je skraca): zmiana wdrożeń albo
                // projektu przebudowuje adapter bez restartu.
                format!(
                    "{}|{}|{:?}|{:?}|{:?}|{}",
                    a.provider,
                    a.id,
                    a.base_url,
                    self.model_of(a),
                    a.queues,
                    serde_json::to_string(&a.enterprise).unwrap_or_default()
                )
            })
            .chain(std::iter::once(format!("{shared:?}")))
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
            // Inne konto pod tym samym dostawcą ma własne limity — pamięć Routera o limicie
            // poprzedniego konta nie może go blokować.
            let switched = self
                .lock()
                .registered
                .get(provider_id)
                .is_some_and(|before| before.account != account.id.as_str());
            for core in &self.cores {
                core.register(provider.clone(), RouteKind::Api);
                if switched {
                    core.reset_health(&ProviderId::new(provider_id.as_str()));
                }
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
        let global = global_candidates(&shared, &chosen);
        for core in &self.cores {
            core.set_provider_queues(queues.clone());
            core.set_global_queues(global.clone());
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
