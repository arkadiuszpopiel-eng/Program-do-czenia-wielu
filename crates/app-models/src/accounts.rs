//! Konta i klucze API w IPC (PLAN §5.6; wydzielone z `app-core` przez limit rozmiaru crate'a):
//! odwzorowanie wpisów katalogu dostawców i kont huba na DTO (bez sekretu), klasy zadań po
//! nazwach z interfejsu i termin okresowego sprawdzenia klucza (plan v6, C2.14).

/// Zmiana ustawień firmowych istniejącego konta (`accounts_set_enterprise`, C2.12).
#[path = "accounts_enterprise.rs"]
pub mod enterprise;

use std::collections::BTreeSet;

use accounts_hub_contract::{
    Account as CoreAccount, AccountErrorKind, AccountState as CoreState, Assignments,
    AuthKind as CoreAuth, Compat, ModelUse, ProviderCatalogEntry, ProviderKind as CoreKind,
    TaskClass, Tribool, VoiceRole,
};
use app_api::AppError;
use app_api::dto::{
    self, Account, AccountAssignment, AccountCostLimit, AccountLastTest, AccountState, AuthKind,
    CompatKind, ComplianceStatus, ModelInfo, ModelKind, Money, ProviderInfo, ProviderKind,
};
use chrono::{DateTime, Utc};
use compliance_contract::ProviderApiStatus;

/// Wklejony tekst nie jest kluczem (po zdjęciu cudzysłowów, `NAZWA=` i podobnych dodatków).
pub const NOT_A_KEY: &str = "To nie wygląda na klucz API: po kluczu zostały spacje albo znaki spoza alfabetu \
                         łacińskiego. Wklej sam klucz, bez opisu i dodatkowego tekstu.";

/// Najdłuższy odstęp okresowego sprawdzania kluczy (tydzień).
pub const MAX_INTERVAL_HOURS: u64 = 168;

/// Klasy zadań i ich nazwy w interfejsie.
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
pub fn task_from_name(name: &str) -> Result<TaskClass, AppError> {
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

/// Model konta → DTO (rodzaje z przeznaczenia modelu i widzenia).
fn model_info(m: &accounts_hub_contract::ModelInfo) -> ModelInfo {
    let purpose = accounts_hub_contract::model_use(m.id.as_str());
    let vision = m
        .capabilities
        .as_ref()
        .is_some_and(|c| c.vision == Tribool::Yes);
    let kinds = match purpose {
        ModelUse::Chat | ModelUse::Reasoning if vision => {
            vec![ModelKind::Chat, ModelKind::Vision]
        }
        ModelUse::Chat | ModelUse::Reasoning => vec![ModelKind::Chat],
        ModelUse::Embedding => vec![ModelKind::Embeddings],
        ModelUse::Speech => vec![ModelKind::Tts],
        ModelUse::Transcription => vec![ModelKind::Stt],
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
        models: a.models.iter().map(model_info).collect(),
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
        last_tested_at: a.last_test.as_ref().map(|t| dto::iso(t.at)),
        // Konto sprzed wprowadzenia pola: klucz jest co najmniej tak stary jak konto.
        key_saved_at: key_stored.then(|| dto::iso(a.key_saved_at.unwrap_or(a.created_at))),
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
        enterprise: a.enterprise.clone(),
        base_url: a.base_url.clone(),
    }
}

/// Przydziały z interfejsu → przydziały huba (`conversation` to dawna nazwa `chat`).
pub fn assignments(a: &AccountAssignment) -> Assignments {
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

/// Czy konto jest do sprawdzenia: ma klucz, nie jest wyłączone, a ostatni test jest starszy niż
/// odstęp (albo nie było go wcale).
pub fn is_due(account: &CoreAccount, now: DateTime<Utc>, interval_hours: u64) -> bool {
    if interval_hours == 0 || matches!(account.state, CoreState::Disabled | CoreState::Unconfigured)
    {
        return false;
    }
    let hours = i64::try_from(interval_hours.min(MAX_INTERVAL_HOURS)).unwrap_or(24);
    account
        .last_test
        .as_ref()
        .is_none_or(|test| now.signed_duration_since(test.at) >= chrono::Duration::hours(hours))
}

#[cfg(test)]
#[path = "accounts_tests.rs"]
mod tests;
