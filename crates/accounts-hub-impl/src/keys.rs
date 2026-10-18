//! Operacje na kluczu istniejącego konta: test, wymiana, ponowne włączenie, model wpisany ręcznie.
//!
//! Wspólna zasada: wynik operacji sieciowej trafia do **bieżącego** wpisu konta (nie do kopii sprzed
//! wywołania), a klucz w magazynie zmienia się dopiero wtedy, gdy nowy został sprawdzony. Wcześniej
//! test zapisywał z powrotem kopię sprzed kilkunastu sekund — usunięte w tym czasie konto wracało,
//! a wymiana klucza niszczyła działający klucz, zanim okazało się, że nowy jest błędny.

use accounts_hub_contract::{
    Account, AccountId, AccountState, AccountsError, AuthKind, EVENT_KEY_ROTATED, EVENT_KEY_TESTED,
    EnterpriseSettings, ModelId, ModelInfo, ProviderId, SecretString, TestSummary,
};
use core_bus_contract::Level;

use crate::hub::AccountsHubService;

fn invalid(message: &str) -> AccountsError {
    AccountsError::InvalidInput(message.to_owned())
}

impl AccountsHubService {
    /// Test połączenia kandydata na klucz i wykrycie modeli (bez zapisu czegokolwiek).
    async fn probe(
        &self,
        acc: &Account,
        secret: Option<&SecretString>,
    ) -> Result<(TestSummary, Option<Vec<ModelInfo>>), AccountsError> {
        let entry = self.provider_entry(&acc.provider)?;
        let base_url = acc.base_url.clone().or_else(|| entry.base_url.clone());
        let report = self
            .run_test(&entry, base_url.as_deref(), secret, &acc.enterprise)
            .await;
        // Klucz bez prawa do listy modeli: drugie zapytanie dałoby tę samą odmowę. Pusta lista
        // nie kasuje tego, co konto już zna (np. model wpisany ręcznie).
        let models = if report.outcome.may_list_models() {
            self.run_listing(&entry, base_url.as_deref(), secret, &acc.enterprise)
                .await
                .ok()
                .filter(|found| !found.is_empty())
        } else {
            None
        };
        let summary = TestSummary {
            at: self.now(),
            outcome: report.outcome,
            latency_ms: report.latency_ms,
            models_found: models.as_ref().map(Vec::len),
            detail: report.detail,
        };
        Ok((summary, models))
    }

    /// Zapisuje wynik testu w bieżącym wpisie konta. `None`, gdy konto w międzyczasie zniknęło
    /// albo dostało inny klucz — wynik dotyczy wtedy czegoś, czego już nie ma.
    fn store_test(
        &self,
        id: &AccountId,
        tested: Option<&SecretString>,
        summary: &TestSummary,
        models: Option<&Vec<ModelInfo>>,
    ) -> Result<Option<(Account, AccountState)>, AccountsError> {
        self.update_accounts(|accounts| {
            let Some(current) = accounts.get_mut(id) else {
                return Ok(None);
            };
            let stored = self.secrets.get(&current.secret)?;
            let same_key = match (&stored, tested) {
                (Some(a), Some(b)) => a.ct_eq(b),
                (None, None) => true,
                _ => false,
            };
            if !same_key {
                return Ok(None);
            }
            let from = current.state.clone();
            current.state = summary.outcome.next_state(&current.state);
            current.last_test = Some(summary.clone());
            if let Some(models) = models {
                current.models.clone_from(models);
            }
            Ok(Some((current.clone(), from)))
        })
    }

    pub(crate) async fn test_account_now(
        &self,
        id: &AccountId,
    ) -> Result<TestSummary, AccountsError> {
        let acc = self.account_or_err(id)?;
        let entry = self.provider_entry(&acc.provider)?;
        // Dostawca zabroniony po dodaniu konta: klucz nie wychodzi już do niego nawet na test.
        self.ensure_allowed(&entry)?;
        let secret = self.secret_of(&acc)?;
        if entry.auth == AuthKind::ApiKey && secret.is_none() {
            let changed = self.update_accounts(|accounts| {
                Ok(accounts.get_mut(id).map(|current| {
                    let from = current.state.clone();
                    if from != AccountState::Disabled {
                        current.state = AccountState::Unconfigured;
                    }
                    (current.clone(), from)
                }))
            })?;
            if let Some((current, from)) = changed {
                self.publish_state(&current, &from).await;
            }
            return Err(AccountsError::SecretMissing(id.clone()));
        }
        let (summary, models) = self.probe(&acc, secret.as_ref()).await?;
        let Some((current, from)) =
            self.store_test(id, secret.as_ref(), &summary, models.as_ref())?
        else {
            return Ok(summary);
        };
        let payload = serde_json::json!({
            "account": current.id, "provider": current.provider,
            "outcome": summary.outcome.code(), "latency_ms": summary.latency_ms,
            "models": summary.models_found,
        });
        self.publish_state(&current, &from).await;
        self.publish(EVENT_KEY_TESTED, Level::Info, payload).await;
        Ok(summary)
    }

    /// Wymiana klucza: najpierw test kandydata, zapis dopiero gdy dostawca go rozpoznał albo nie
    /// da się go sprawdzić ([`accounts_hub_contract::TestOutcome::accepts_new_key`]). W pozostałych
    /// przypadkach konto i stary klucz zostają nietknięte, a wynik mówi, co było nie tak z nowym.
    pub(crate) async fn try_rotate_now(
        &self,
        id: &AccountId,
        secret: SecretString,
    ) -> Result<TestSummary, AccountsError> {
        let acc = self.account_or_err(id)?;
        let entry = self.provider_entry(&acc.provider)?;
        if entry.auth != AuthKind::ApiKey {
            return Err(invalid("to konto nie używa klucza API"));
        }
        self.ensure_allowed(&entry)?;
        if !secret.is_plausible_key() {
            return Err(invalid("klucz jest pusty albo zawiera niedozwolone znaki"));
        }
        let (summary, models) = self.probe(&acc, Some(&secret)).await?;
        if !summary.outcome.accepts_new_key() {
            return Ok(summary);
        }
        let previous = self.secret_of(&acc)?;
        self.secrets.put(&acc.secret, &secret)?;
        let saved_at = self.now();
        let stored = self.update_accounts(|accounts| {
            let current = accounts
                .get_mut(id)
                .ok_or_else(|| AccountsError::UnknownAccount(id.clone()))?;
            let from = current.state.clone();
            current.state = summary.outcome.next_state(&current.state);
            current.last_test = Some(summary.clone());
            current.key_saved_at = Some(saved_at);
            if let Some(models) = &models {
                current.models.clone_from(models);
            }
            Ok((current.clone(), from))
        });
        let (current, from) = match stored {
            Ok(done) => done,
            Err(error) => {
                // Metadane się nie zapisały — w magazynie ma zostać klucz, który do nich pasuje.
                let _ = match &previous {
                    Some(old) => self.secrets.put(&acc.secret, old),
                    None => self.secrets.delete(&acc.secret).map(drop),
                };
                return Err(error);
            }
        };
        let payload = serde_json::json!({ "account": current.id, "provider": current.provider });
        self.publish_state(&current, &from).await;
        self.publish(EVENT_KEY_ROTATED, Level::Info, payload).await;
        Ok(summary)
    }

    /// Włączenie przywraca stan sprzed wyłączenia (z ostatniego testu); wyłączenie zachowuje klucz.
    /// Włączanie konta, które nie jest wyłączone, niczego nie zmienia — wcześniej każde „włącz”
    /// robiło konto aktywnym, także z kluczem odrzuconym przez dostawcę.
    pub(crate) async fn set_disabled_now(
        &self,
        id: &AccountId,
        disabled: bool,
    ) -> Result<(), AccountsError> {
        let acc = self.account_or_err(id)?;
        let has_key = self.secret_of(&acc)?.is_some()
            || self.provider_entry(&acc.provider)?.auth == AuthKind::None;
        let changed = self.update_accounts(|accounts| {
            let current = accounts
                .get_mut(id)
                .ok_or_else(|| AccountsError::UnknownAccount(id.clone()))?;
            let from = current.state.clone();
            if disabled {
                current.state = AccountState::Disabled;
            } else if from == AccountState::Disabled {
                current.state = match (&current.last_test, has_key) {
                    (_, false) => AccountState::Unconfigured,
                    (Some(test), true) => test.outcome.next_state(&AccountState::Active),
                    (None, true) => AccountState::Active,
                };
            }
            Ok((current.clone(), from))
        })?;
        self.publish_state(&changed.0, &changed.1).await;
        Ok(())
    }

    /// Model podany ręcznie — dla klucza, który nie ma prawa do listy modeli (bez modelu Router
    /// nie ma czego wywołać). Następny udany odczyt listy modeli go zastąpi.
    pub(crate) async fn set_manual_model_now(
        &self,
        id: &AccountId,
        model: ModelId,
    ) -> Result<(), AccountsError> {
        self.update_accounts(|accounts| {
            let current = accounts
                .get_mut(id)
                .ok_or_else(|| AccountsError::UnknownAccount(id.clone()))?;
            // Lista od dostawcy zostaje: model spoza niej to najpewniej literówka.
            if current.models.len() > 1 {
                return if current.models.iter().any(|m| m.id == model) {
                    Ok(())
                } else {
                    Err(invalid(
                        "to konto ma listę modeli od dostawcy — wybierz model z listy",
                    ))
                };
            }
            current.models = vec![ModelInfo {
                id: model,
                display_name: None,
                capabilities: None,
                context_window: None,
            }];
            Ok(())
        })
    }

    /// Kolejka modeli klasy zadań (pusta = wybór automatyczny).
    pub(crate) async fn set_model_queue_now(
        &self,
        id: &AccountId,
        class: accounts_hub_contract::TaskClass,
        models: Vec<ModelId>,
    ) -> Result<(), AccountsError> {
        let names: Vec<String> = models.iter().map(ToString::to_string).collect();
        accounts_hub_contract::validate_queue(class, &names)
            .map_err(AccountsError::InvalidInput)?;
        let changed = self.update_accounts(|accounts| {
            let current = accounts
                .get_mut(id)
                .ok_or_else(|| AccountsError::UnknownAccount(id.clone()))?;
            if models.is_empty() {
                current.queues.remove(&class);
            } else {
                current.queues.insert(class, models);
            }
            Ok(current.clone())
        })?;
        // Router przelicza trasy po zmianie stanu konta — ten sam sygnał niesie nową kolejkę.
        self.publish_state(&changed, &changed.state).await;
        Ok(())
    }

    /// Konto tego dostawcy pod tym samym adresem, które ma już dokładnie ten klucz.
    pub(crate) fn find_same_key(
        &self,
        provider: &ProviderId,
        base_url: Option<&str>,
        enterprise: &EnterpriseSettings,
        secret: &SecretString,
    ) -> Option<AccountId> {
        if secret.is_empty() {
            return None;
        }
        let default_url = self.provider_entry(provider).ok()?.base_url;
        let effective = |url: Option<&str>| {
            url.map(str::to_owned)
                .or_else(|| default_url.clone())
                .map(|u| u.trim_end_matches('/').to_ascii_lowercase())
        };
        let wanted = effective(base_url);
        let accounts: Vec<Account> = self.accounts_read().values().cloned().collect();
        accounts
            .into_iter()
            .filter(|acc| &acc.provider == provider && effective(acc.base_url.as_deref()) == wanted)
            .filter(|acc| acc.enterprise.billing_project() == enterprise.billing_project())
            .find(|acc| matches!(self.secret_of(acc), Ok(Some(stored)) if stored.ct_eq(secret)))
            .map(|acc| acc.id)
    }
}
