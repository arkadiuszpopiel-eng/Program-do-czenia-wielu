//! Komendy `accounts_*`: Hub kont i kluczy (PLAN §5.6). Sekret przechodzi przez IPC tylko
//! w `accounts_add` i trafia wyłącznie do Credential Managera; do UI wraca `key_stored`.

use std::collections::BTreeSet;

use accounts_hub_contract::{
    Account as CoreAccount, AccountErrorKind, AccountId, AccountSource, AccountState as CoreState,
    AccountsHub, Assignments, AuthKind as CoreAuth, Compat, CostLimit, NewAccount,
    ProviderCatalogEntry, ProviderId, ProviderKind as CoreKind, SecretString, TaskClass, Tribool,
    VoiceRole,
};
use compliance_contract::ProviderApiStatus;

use crate::core::AppCore;
use crate::dto::{
    Account, AccountAssignment, AccountCostLimit, AccountState, AddAccountInput, AlfaEvent,
    AuthKind, CompatKind, ComplianceStatus, ModelInfo, ModelKind, Money, ProviderInfo,
    ProviderKind, TestReport,
};
use crate::error::AppError;

const TASKS: [(TaskClass, &str); 7] = [
    (TaskClass::Conversation, "chat"),
    (TaskClass::Code, "code"),
    (TaskClass::Planning, "planning"),
    (TaskClass::VoiceFast, "voice_fast"),
    (TaskClass::GuiVision, "gui_vision"),
    (TaskClass::Summarize, "summarize"),
    (TaskClass::Embeddings, "embeddings"),
];

/// Wpis katalogu → DTO.
pub fn provider_info(e: &ProviderCatalogEntry) -> ProviderInfo {
    ProviderInfo {
        id: e.id.to_string(),
        display_name: e.display_name.clone(),
        kind: match e.kind {
            CoreKind::Chat => ProviderKind::Chat,
            CoreKind::Stt => ProviderKind::Stt,
            CoreKind::Tts => ProviderKind::Tts,
            CoreKind::Multi => ProviderKind::Multi,
        },
        auth: match e.auth {
            CoreAuth::ApiKey => AuthKind::ApiKey,
            CoreAuth::OauthCli => AuthKind::OauthCli,
            CoreAuth::None => AuthKind::None,
        },
        compat: match e.compat {
            Compat::Openai => CompatKind::Openai,
            Compat::Anthropic => CompatKind::Anthropic,
            Compat::Native => CompatKind::Native,
        },
        privacy_tag: e.privacy_tag.as_str().to_owned(),
        jurisdiction: e.jurisdiction.to_string(),
        compliance_status: match e.compliance_status {
            ProviderApiStatus::Green => ComplianceStatus::Green,
            ProviderApiStatus::Gray => ComplianceStatus::Gray,
            ProviderApiStatus::Forbidden => ComplianceStatus::Forbidden,
            ProviderApiStatus::Unverified => ComplianceStatus::Unverified,
        },
        terms_url: e.terms_url.clone(),
        needs_base_url: e.base_url.is_none(),
    }
}

/// Konto → DTO (bez sekretu).
pub fn account_dto(a: &CoreAccount) -> Account {
    let state = match &a.state {
        CoreState::Unconfigured => AccountState::Unconfigured,
        CoreState::Active => AccountState::Ok,
        CoreState::Error {
            kind: AccountErrorKind::RateLimited,
        } => AccountState::RateLimited,
        CoreState::Error { .. } => AccountState::Invalid,
        CoreState::Disabled => AccountState::Disabled,
    };
    let key_stored = !matches!(
        a.state,
        CoreState::Unconfigured
            | CoreState::Error {
                kind: AccountErrorKind::SecretMissing
            }
    );
    let limit = a.cost_limit.map_or(
        AccountCostLimit {
            enabled: false,
            monthly: Money::pln(0),
        },
        |l| AccountCostLimit {
            enabled: l.enabled,
            monthly: Money::pln(i64::try_from(l.monthly_limit_grosze).unwrap_or(i64::MAX)),
        },
    );
    Account {
        id: a.id.to_string(),
        provider_id: a.provider.to_string(),
        label: a.label.clone(),
        state,
        key_stored,
        models: a
            .models
            .iter()
            .map(|m| ModelInfo {
                id: m.id.to_string(),
                name: m.display_name.clone().unwrap_or_else(|| m.id.to_string()),
                kinds: if m
                    .capabilities
                    .as_ref()
                    .is_some_and(|c| c.vision == Tribool::Yes)
                {
                    vec![ModelKind::Chat, ModelKind::Vision]
                } else {
                    vec![ModelKind::Chat]
                },
                context_tokens: m.context_window.map(u64::from),
            })
            .collect(),
        assignments: AccountAssignment {
            task_classes: TASKS
                .iter()
                .filter(|(t, _)| a.assignments.task_classes.contains(t))
                .map(|(_, n)| (*n).to_owned())
                .collect(),
            agents: a.assignments.agents.iter().cloned().collect(),
            voice_stt: a.assignments.voice.contains(&VoiceRole::Stt),
            voice_tts: a.assignments.voice.contains(&VoiceRole::Tts),
        },
        cost_limit: limit,
        last_tested_at: a.last_test.as_ref().map(|t| crate::dto::iso(t.at)),
    }
}

fn assignments(a: &AccountAssignment) -> Assignments {
    let mut voice = BTreeSet::new();
    if a.voice_stt {
        voice.insert(VoiceRole::Stt);
    }
    if a.voice_tts {
        voice.insert(VoiceRole::Tts);
    }
    Assignments {
        task_classes: TASKS
            .iter()
            .filter(|(_, n)| {
                a.task_classes
                    .iter()
                    .any(|x| x == n || (x == "conversation" && *n == "chat"))
            })
            .map(|(t, _)| *t)
            .collect(),
        agents: a.agents.iter().cloned().collect(),
        voice,
    }
}

fn account_id(id: &str) -> Result<AccountId, AppError> {
    AccountId::new(id).map_err(AppError::from)
}

impl AppCore {
    fn account(&self, id: &AccountId) -> Result<CoreAccount, AppError> {
        self.inner
            .hub
            .account(id)
            .ok_or_else(|| AppError::not_found(format!("Konto „{id}” nie istnieje.")))
    }

    async fn announce_account(&self, id: &AccountId) {
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
        let entry = self
            .inner
            .hub
            .provider(&provider)
            .ok_or_else(|| AppError::not_found(format!("Nieznany dostawca „{provider}”.")))?;
        let secret = SecretString::from_input(input.secret.expose());
        if entry.auth == CoreAuth::ApiKey && !secret.is_plausible_key() {
            return Err(AppError::invalid(
                "Klucz API jest pusty albo zawiera spacje lub znaki sterujące.",
            ));
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
                base_url: input.base_url.filter(|u| !u.trim().is_empty()),
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
        let summary = self.inner.hub.test_account(&id).await?;
        self.announce_account(&id).await;
        let account = account_dto(&self.account(&id)?);
        let ok = summary.outcome.key_works();
        let error = (!ok).then(|| match &summary.outcome {
            accounts_hub_contract::TestOutcome::InvalidKey => "Klucz odrzucony (401).".to_owned(),
            accounts_hub_contract::TestOutcome::Timeout => {
                "Przekroczony czas połączenia.".to_owned()
            }
            accounts_hub_contract::TestOutcome::Network { message }
            | accounts_hub_contract::TestOutcome::Unsupported { message } => message.clone(),
            other => other.code().to_owned(),
        });
        Ok(TestReport {
            ok,
            latency_ms: summary.latency_ms,
            models: account.models,
            error,
        })
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
