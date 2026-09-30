//! Konto (klucz) dostawcy: stan, przypisania, limit kosztów, wynik ostatniego testu.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::ModelInfo;
use crate::ids::{AccountId, ProviderId, SecretName};
use crate::secret::SecretString;

/// Stan konta (także stan dostawcy w UI: „nieskonfigurowany”, „aktywny”, „błąd”, „wyłączony”).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AccountState {
    /// Brak klucza (np. metadane przeniesione z innej maszyny) albo dostawca bez kont.
    Unconfigured,
    /// Gotowe do użycia przez Router.
    Active,
    /// Ostatni test lub użycie zakończone błędem.
    Error {
        /// Rodzaj błędu.
        kind: AccountErrorKind,
    },
    /// Wyłączone przez użytkownika.
    Disabled,
}

impl AccountState {
    /// Czy Router może używać konta.
    pub fn is_usable(&self) -> bool {
        matches!(self, AccountState::Active)
    }
}

/// Rodzaj błędu konta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountErrorKind {
    /// Klucz odrzucony (401/403).
    InvalidKey,
    /// Limit zapytań.
    RateLimited,
    /// Błąd sieci.
    Network,
    /// Przekroczony czas testu.
    Timeout,
    /// Brak sekretu w magazynie.
    SecretMissing,
}

/// Wynik testu połączenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum TestOutcome {
    /// Połączenie i uwierzytelnienie poprawne.
    Ok,
    /// Klucz odrzucony.
    InvalidKey,
    /// Limit zapytań (klucz poprawny).
    RateLimited,
    /// Błąd sieci (komunikat bez sekretów).
    Network {
        /// Opis.
        message: String,
    },
    /// Przekroczony czas.
    Timeout,
    /// Tester nie obsługuje tego dostawcy/konfiguracji.
    Unsupported {
        /// Opis.
        message: String,
    },
}

impl TestOutcome {
    /// Czy klucz można uznać za działający (także przy limicie zapytań).
    pub fn key_works(&self) -> bool {
        matches!(self, TestOutcome::Ok | TestOutcome::RateLimited)
    }

    /// Stan konta po teście (wyłączone konto zostaje wyłączone; `Unsupported` nie zmienia stanu).
    pub fn next_state(&self, current: &AccountState) -> AccountState {
        if *current == AccountState::Disabled {
            return AccountState::Disabled;
        }
        let error = |kind| AccountState::Error { kind };
        match self {
            TestOutcome::Ok | TestOutcome::RateLimited => AccountState::Active,
            TestOutcome::InvalidKey => error(AccountErrorKind::InvalidKey),
            TestOutcome::Network { .. } => error(AccountErrorKind::Network),
            TestOutcome::Timeout => error(AccountErrorKind::Timeout),
            TestOutcome::Unsupported { .. } => current.clone(),
        }
    }

    /// Kod tekstowy (do zdarzeń).
    pub fn code(&self) -> &'static str {
        match self {
            TestOutcome::Ok => "ok",
            TestOutcome::InvalidKey => "invalid_key",
            TestOutcome::RateLimited => "rate_limited",
            TestOutcome::Network { .. } => "network",
            TestOutcome::Timeout => "timeout",
            TestOutcome::Unsupported { .. } => "unsupported",
        }
    }
}

/// Raport testera połączenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConnectionReport {
    /// Wynik.
    pub outcome: TestOutcome,
    /// Opóźnienie w ms, jeśli zmierzone.
    pub latency_ms: Option<u64>,
}

/// Podsumowanie ostatniego testu konta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestSummary {
    /// Kiedy.
    pub at: DateTime<Utc>,
    /// Wynik.
    pub outcome: TestOutcome,
    /// Opóźnienie.
    pub latency_ms: Option<u64>,
    /// Liczba wykrytych modeli (jeśli wykrywanie się udało).
    pub models_found: Option<usize>,
}

/// Klasy zadań Routera (PLAN §5.4).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    /// Głos — szybka odpowiedź.
    VoiceFast,
    /// Rozmowa.
    Conversation,
    /// Kod.
    Code,
    /// Planowanie.
    Planning,
    /// GUI / wizja.
    GuiVision,
    /// Streszczanie.
    Summarize,
    /// Embeddingi.
    Embeddings,
}

/// Role głosowe.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum VoiceRole {
    /// Rozpoznawanie mowy.
    Stt,
    /// Synteza mowy.
    Tts,
}

/// Przypisania konta do klas zadań, agentek/ról i głosu.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Assignments {
    /// Klasy zadań.
    pub task_classes: BTreeSet<TaskClass>,
    /// Agentki/role (identyfikatory person).
    pub agents: BTreeSet<String>,
    /// Role głosowe.
    pub voice: BTreeSet<VoiceRole>,
}

/// Limit kosztów per dostawca/konto (opcjonalny, wyłączalny; egzekwuje `cost-meter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CostLimit {
    /// Czy limit blokuje (wyłączony = tylko wskaźnik i alerty).
    pub enabled: bool,
    /// Limit miesięczny w groszach (1 PLN = 100).
    pub monthly_limit_grosze: u64,
}

/// Skąd pochodzi klucz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum AccountSource {
    /// Kreator.
    Wizard,
    /// Import ze zmiennej środowiskowej (nazwa zmiennej, nigdy wartość).
    Env {
        /// Nazwa zmiennej.
        var: String,
    },
    /// Bezpośrednie dodanie przez API modułu.
    Manual,
}

/// Konto (metadane bez sekretu — bezpieczne do konfiguracji i eksportu `.alfa`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Account {
    /// Identyfikator.
    pub id: AccountId,
    /// Dostawca.
    pub provider: ProviderId,
    /// Etykieta użytkownika.
    pub label: String,
    /// Kiedy utworzono.
    pub created_at: DateTime<Utc>,
    /// Ostatni test.
    pub last_test: Option<TestSummary>,
    /// Stan.
    pub state: AccountState,
    /// Uchwyt sekretu w magazynie (nie wartość).
    pub secret: SecretName,
    /// Endpoint nadpisany przez użytkownika (np. własny endpoint).
    pub base_url: Option<String>,
    /// Przypisania.
    pub assignments: Assignments,
    /// Limit kosztów.
    pub cost_limit: Option<CostLimit>,
    /// Pochodzenie klucza.
    pub source: AccountSource,
    /// Modele wykryte przy ostatnim udanym teście.
    pub models: Vec<ModelInfo>,
}

/// Dane nowego konta.
#[derive(Debug, Clone)]
pub struct NewAccount {
    /// Dostawca.
    pub provider: ProviderId,
    /// Etykieta.
    pub label: String,
    /// Klucz (dla `auth = none` może być pusty).
    pub secret: SecretString,
    /// Endpoint (wymagany, gdy katalog go nie zna).
    pub base_url: Option<String>,
    /// Przypisania.
    pub assignments: Assignments,
    /// Limit kosztów.
    pub cost_limit: Option<CostLimit>,
    /// Pochodzenie.
    pub source: AccountSource,
}

/// Stan dostawcy wyliczony z jego kont: aktywne > błąd > wyłączone > nieskonfigurowane.
pub fn provider_state<'a, I: IntoIterator<Item = &'a Account>>(accounts: I) -> AccountState {
    let states: Vec<&AccountState> = accounts.into_iter().map(|a| &a.state).collect();
    if states.iter().any(|s| s.is_usable()) {
        return AccountState::Active;
    }
    if let Some(err) = states
        .iter()
        .find(|s| matches!(s, AccountState::Error { .. }))
    {
        return (*err).clone();
    }
    if !states.is_empty() && states.iter().all(|s| **s == AccountState::Disabled) {
        return AccountState::Disabled;
    }
    AccountState::Unconfigured
}
