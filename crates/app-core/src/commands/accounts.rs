//! Komendy `accounts_*`: Hub kont i kluczy (PLAN §5.6). Sekret przechodzi przez IPC tylko
//! w `accounts_add` i trafia wyłącznie do Credential Managera; do UI wraca `key_stored`.

use accounts_hub_contract::{
    Account as CoreAccount, AccountId, AccountSource, AccountsHub, Assignments,
    AuthKind as CoreAuth, CostLimit, NewAccount, ProviderId, SecretString, TaskClass,
};
// Odwzorowanie kont na DTO — `app_models::accounts` (limit rozmiaru crate'a).
pub(super) use app_models::accounts::{NOT_A_KEY, account_dto, task_from_name};
use app_models::accounts::{assignments, provider_info};

use crate::core::AppCore;
use crate::dto::{
    Account, AccountAssignment, AddAccountInput, AlfaEvent, Money, ProviderInfo, TestReport,
};
use crate::error::AppError;

pub(super) fn account_id(id: &str) -> Result<AccountId, AppError> {
    AccountId::new(id).map_err(AppError::from)
}

impl AppCore {
    pub(super) fn account(&self, id: &AccountId) -> Result<CoreAccount, AppError> {
        self.inner.hub.account(id).ok_or_else(|| {
            AppError::missing(
                "account",
                id.to_string(),
                format!("Konto „{id}” nie istnieje."),
            )
        })
    }

    pub(crate) async fn announce_account(&self, id: &AccountId) {
        if let Some(a) = self.inner.hub.account(id) {
            self.emit(AlfaEvent::AccountChanged {
                account: account_dto(&a),
            });
        }
        let status = self.status_snapshot().await;
        self.emit(AlfaEvent::SystemStatusChanged { status });
    }

    /// `accounts_catalog`.
    pub async fn accounts_catalog(&self) -> Result<Vec<ProviderInfo>, AppError> {
        Ok(self.inner.hub.catalog().iter().map(provider_info).collect())
    }

    /// `accounts_list`.
    pub async fn accounts_list(&self) -> Result<Vec<Account>, AppError> {
        Ok(self.inner.hub.accounts().iter().map(account_dto).collect())
    }

    /// `accounts_add`: klucz → Credential Manager (nigdy z powrotem do UI).
    pub async fn accounts_add(&self, input: AddAccountInput) -> Result<Account, AppError> {
        let provider = ProviderId::new(input.provider_id.as_str())?;
        let entry = self.inner.hub.provider(&provider).ok_or_else(|| {
            AppError::missing(
                "provider",
                provider.to_string(),
                format!("Nieznany dostawca „{provider}”."),
            )
        })?;
        let secret = SecretString::from_input(input.secret.expose());
        if entry.auth == CoreAuth::ApiKey && !secret.is_plausible_key() {
            return Err(AppError::invalid(NOT_A_KEY));
        }
        let base = input
            .base_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty());
        let hub = &self.inner.hub;
        if let Some(existing) = hub.find_by_secret(&provider, base, &input.enterprise, &secret) {
            let label = hub
                .account(&existing)
                .map_or_else(|| existing.to_string(), |a| a.label);
            return Err(AppError::invalid(format!(
                "Ten klucz jest już dodany jako konto „{label}”. Jeśli chcesz go sprawdzić, użyj „Testuj połączenie” przy tym koncie."
            )));
        }
        let label = match input.label.trim() {
            "" => entry.display_name.clone(),
            l => l.to_owned(),
        };
        let id = self
            .inner
            .hub
            .add_account(NewAccount {
                provider,
                label,
                secret,
                base_url: base.map(str::to_owned),
                enterprise: input.enterprise,
                assignments: Assignments {
                    task_classes: [TaskClass::Conversation].into(),
                    ..Assignments::default()
                },
                cost_limit: None,
                source: AccountSource::Wizard,
            })
            .await?;
        self.announce_account(&id).await;
        Ok(account_dto(&self.account(&id)?))
    }

    /// `accounts_test`: połączenie + wykrycie modeli.
    pub async fn accounts_test(&self, account_id: String) -> Result<TestReport, AppError> {
        let id = self::account_id(&account_id)?;
        let tested = self.inner.hub.test_account(&id).await;
        // Także po odmowie huba (np. brak klucza w magazynie) stan konta mógł się zmienić.
        self.announce_account(&id).await;
        let summary = tested?;
        Ok(self.report(&id, &summary))
    }

    /// `accounts_assign`.
    pub async fn accounts_assign(
        &self,
        account_id: String,
        assignment: AccountAssignment,
    ) -> Result<(), AppError> {
        let id = self::account_id(&account_id)?;
        let current = self.account(&id)?;
        self.inner
            .hub
            .update_settings(
                &id,
                &current.label,
                assignments(&assignment),
                current.cost_limit,
            )
            .await?;
        self.announce_account(&id).await;
        Ok(())
    }

    /// `accounts_set_limit`.
    pub async fn accounts_set_limit(
        &self,
        account_id: String,
        enabled: bool,
        monthly: Money,
    ) -> Result<(), AppError> {
        let id = self::account_id(&account_id)?;
        let current = self.account(&id)?;
        let limit = CostLimit {
            enabled,
            monthly_limit_grosze: u64::try_from(monthly.minor)
                .map_err(|_| AppError::invalid("Limit nie może być ujemny."))?,
        };
        self.inner
            .hub
            .update_settings(
                &id,
                &current.label,
                current.assignments.clone(),
                Some(limit),
            )
            .await?;
        self.announce_account(&id).await;
        Ok(())
    }

    /// `accounts_remove`: usuwa konto i wpis Credential Managera.
    pub async fn accounts_remove(&self, account_id: String) -> Result<(), AppError> {
        let id = self::account_id(&account_id)?;
        self.inner.hub.remove(&id).await?;
        let status = self.status_snapshot().await;
        self.emit(AlfaEvent::SystemStatusChanged { status });
        Ok(())
    }
}
