//! Domyślny „mózg" do czasu podłączenia modułu `router`: pierwszy skonfigurowany dostawca czatu
//! z `accounts-hub` (najpierw konta przypisane agentce), adapter z `providers-api-impl`, model
//! z wykrytych przy teście konta (albo z Models API przy pierwszym użyciu). Bez kluczy — stan
//! „brak mózgu". Profil lokalny wymaga `providers-local` (niepodłączony).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use accounts_hub_contract::{Account, AccountsHub, ProviderCatalogEntry, ProviderKind};
use accounts_hub_impl::AccountsHubService;
use async_trait::async_trait;
use providers_api_impl::{AccountProfile, CatalogEntry, build_provider};
use providers_contract::{ModelKind, ModelProvider};
use sessions_contract::PrivacyTag;

use crate::dto::ModelProfile;
use crate::infra::secrets::HubKeySource;
use crate::ports::{BrainChoice, BrainError, BrainPort, BrainRequest};

/// Komunikat „brak mózgu" (PLAN §14.5: bez kluczy startuje profil lokalny).
pub const NO_BRAIN: &str = "Brak mózgu: dodaj klucz API w Ustawieniach → Modele i dostawcy \
     albo pobierz model lokalny.";
/// Profil lokalny bez modułu `providers-local`.
pub const NO_LOCAL: &str = "Profil lokalny: model lokalny (moduł providers-local) nie jest jeszcze \
     podłączony — dodaj klucz API albo przełącz profil na Hybrydę.";

struct Cached {
    provider: Arc<dyn ModelProvider>,
    model: Option<String>,
}

/// Wybór: pierwszy skonfigurowany `ModelProvider`.
pub struct DirectBrain {
    hub: Arc<AccountsHubService>,
    catalog: Arc<BTreeMap<String, CatalogEntry>>,
    cache: Mutex<HashMap<String, Cached>>,
}

impl DirectBrain {
    /// Mózg na hubie kont i katalogu adapterów.
    pub fn new(hub: Arc<AccountsHubService>, catalog: Arc<BTreeMap<String, CatalogEntry>>) -> Self {
        Self {
            hub,
            catalog,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn entry(&self, account: &Account) -> Option<ProviderCatalogEntry> {
        self.hub.provider(&account.provider)
    }

    fn eligible(&self, privacy: PrivacyTag) -> Vec<Account> {
        let mut accounts: Vec<Account> = self
            .hub
            .accounts()
            .into_iter()
            .filter(|a| a.state.is_usable())
            .filter(|a| self.catalog.contains_key(a.provider.as_str()))
            .filter(|a| {
                self.entry(a).is_some_and(|e| {
                    matches!(e.kind, ProviderKind::Chat | ProviderKind::Multi)
                        && (privacy != PrivacyTag::Private || private_ok(&e))
                })
            })
            .collect();
        accounts.sort_by_key(|a| a.created_at);
        accounts
    }

    fn provider_for(&self, account: &Account) -> Result<Arc<dyn ModelProvider>, BrainError> {
        let key = account.id.as_str().to_owned();
        if let Some(c) = self.lock().get(&key) {
            return Ok(c.provider.clone());
        }
        let entry = self
            .catalog
            .get(account.provider.as_str())
            .ok_or_else(|| BrainError::Provider(format!("brak adaptera `{}`", account.provider)))?;
        let hub: Arc<dyn AccountsHub> = self.hub.clone();
        let mut profile = AccountProfile::new(Arc::new(HubKeySource::new(hub, account.id.clone())));
        profile.base_url = account.base_url.clone();
        let provider = build_provider(entry, profile)
            .map_err(|e| BrainError::Provider(format!("{}: {e}", account.provider)))?;
        self.lock().insert(
            key,
            Cached {
                provider: provider.clone(),
                model: None,
            },
        );
        Ok(provider)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Cached>> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }

    async fn model_for(
        &self,
        account: &Account,
        provider: &Arc<dyn ModelProvider>,
    ) -> Result<(String, Option<u64>), BrainError> {
        if let Some(m) = account.models.first() {
            return Ok((m.id.as_str().to_owned(), m.context_window.map(u64::from)));
        }
        let cached = self
            .lock()
            .get(account.id.as_str())
            .and_then(|c| c.model.clone());
        if let Some(model) = cached {
            return Ok((model, None));
        }
        let models = provider
            .list_models()
            .await
            .map_err(|e| BrainError::Provider(format!("wykrycie modeli: {e}")))?;
        let chosen = models
            .into_iter()
            .find(|m| {
                m.capabilities
                    .as_ref()
                    .is_none_or(|c| c.kinds.is_empty() || c.kinds.contains(&ModelKind::Chat))
            })
            .ok_or_else(|| BrainError::Provider("dostawca nie zwrócił żadnego modelu".into()))?;
        let window = chosen
            .capabilities
            .as_ref()
            .and_then(|c| c.context_window)
            .map(u64::from);
        if let Some(c) = self.lock().get_mut(account.id.as_str()) {
            c.model = Some(chosen.id.clone());
        }
        Ok((chosen.id, window))
    }
}

/// Sesja prywatna: bez tras „może trenować" i jurysdykcji CN (PLAN §5.5).
fn private_ok(entry: &ProviderCatalogEntry) -> bool {
    let tag = entry.privacy_tag.as_str();
    !tag.contains("may-train") && !entry.jurisdiction.to_string().contains("CN")
}

#[async_trait]
impl BrainPort for DirectBrain {
    async fn choose(&self, request: &BrainRequest) -> Result<BrainChoice, BrainError> {
        if request.profile == Some(ModelProfile::Local) || request.privacy == PrivacyTag::LocalOnly
        {
            return Err(BrainError::NoKeys(NO_LOCAL.into()));
        }
        let accounts = self.eligible(request.privacy);
        let assigned = accounts
            .iter()
            .find(|a| a.assignments.agents.contains(&request.agent))
            .or_else(|| accounts.first())
            .ok_or_else(|| BrainError::NoKeys(NO_BRAIN.into()))?;
        let provider = self.provider_for(assigned)?;
        let (model, context_window) = self.model_for(assigned, &provider).await?;
        let name = self
            .entry(assigned)
            .map_or_else(|| assigned.provider.to_string(), |e| e.display_name);
        Ok(BrainChoice {
            provider,
            provider_id: assigned.provider.to_string(),
            provider_name: name,
            account: Some(assigned.id.to_string()),
            model,
            context_window,
        })
    }

    fn keys_configured(&self) -> bool {
        !self.eligible(PrivacyTag::Normal).is_empty()
    }
}
