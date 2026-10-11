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
    Account, AccountAssignment, AccountCostLimit, AccountLastTest, AccountState, AddAccountInput,
    AlfaEvent, AuthKind, CompatKind, ComplianceStatus, ModelInfo, ModelKind, Money, ProviderInfo,
    ProviderKind, TestReport,
};
use crate::error::AppError;

/// Wklejony tekst nie jest kluczem (po zdjęciu cudzysłowów, `NAZWA=` i podobnych dodatków).
pub(super) const NOT_A_KEY: &str = "To nie wygląda na klucz API: po kluczu zostały spacje albo znaki spoza alfabetu \
                         łacińskiego. Wklej sam klucz, bez opisu i dodatkowego tekstu.";

const TASKS: [(TaskClass, &str); 7] = [
    (TaskClass::Conversation, "chat"),
    (TaskClass::Code, "code"),
    (TaskClass::Planning, "planning"),
    (TaskClass::VoiceFast, "voice_fast"),
    (TaskClass::GuiVision, "gui_vision"),
    (TaskClass::Summarize, "summarize"),
    (TaskClass::Embeddings, "embeddings"),
];

/// Nazwa wariantu wyliczenia tak, jak idzie przez IPC (`snake_case`).
fn snake<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Klasa zadań z nazwy używanej w interfejsie (`chat`, `code`, …).
pub(super) fn task_from_name(name: &str) -> Result<TaskClass, AppError> {
    TASKS
        .iter()
        .find(|(_, n)| *n == name)
        .map(|(t, _)| *t)
        .ok_or_else(|| AppError::invalid(format!("Nieznana klasa zadań „{name}”.")))
}

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
            .map(|m| {
                let purpose = accounts_hub_contract::model_use(m.id.as_str());
                let vision = m
                    .capabilities
                    .as_ref()
                    .is_some_and(|c| c.vision == Tribool::Yes);
                use accounts_hub_contract::ModelUse as Use;
                let kinds = match purpose {
                    Use::Chat | Use::Reasoning if vision => {
                        vec![ModelKind::Chat, ModelKind::Vision]
                    }
                    Use::Chat | Use::Reasoning => vec![ModelKind::Chat],
                    Use::Embedding => vec![ModelKind::Embeddings],
                    Use::Speech => vec![ModelKind::Tts],
                    Use::Transcription => vec![ModelKind::Stt],
                    _ => Vec::new(),
                };
                ModelInfo {
                    id: m.id.to_string(),
                    name: m.display_name.clone().unwrap_or_else(|| m.id.to_string()),
                    kinds,
                    context_tokens: m.context_window.map(u64::from),
                    purpose: snake(&purpose),
                    tier: snake(&accounts_hub_contract::model_tier(m.id.as_str())),
                    snapshot: accounts_hub_contract::is_snapshot(m.id.as_str()),
                }
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
        last_test: a.last_test.as_ref().map(|t| AccountLastTest {
            ok: t.outcome.key_works(),
            reason: t.outcome.code().to_owned(),
            latency_ms: t.latency_ms,
            status: t.detail.as_ref().and_then(|d| d.status),
            provider_code: t.detail.as_ref().and_then(|d| d.code.clone()),
            detail: t.detail.as_ref().map(|d| d.message.clone()),
        }),
        queues: TASKS
            .iter()
            .filter_map(|(class, name)| {
                let models = a.queues.get(class).filter(|q| !q.is_empty())?;
                Some((
                    (*name).to_owned(),
                    models.iter().map(ToString::to_string).collect(),
                ))
            })
            .collect(),
        auto_model: accounts_hub_contract::best_chat_model(a.models.iter().map(|m| m.id.as_str()))
            .map(str::to_owned),
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

    pub(super) async fn announce_account(&self, id: &AccountId) {
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
        if let Some(existing) = self.inner.hub.find_by_secret(&provider, base, &secret) {
            let label = self
                .inner
                .hub
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
                base_url: input
                    .base_url
                    .map(|u| u.trim().to_owned())
                    .filter(|u| !u.is_empty()),
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
