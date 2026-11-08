//! Konto (klucz) dostawcy: stan, przypisania, limit kosztów, wynik ostatniego testu.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::catalog::ModelInfo;
use crate::enterprise::EnterpriseSettings;
use crate::ids::{AccountId, ModelId, ProviderId, SecretName};
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
    /// Klucz poprawny, ale z ograniczonymi uprawnieniami (np. bez prawa do listy modeli) —
    /// dostawca odpowiada wtedy 401/403, choć sam klucz jest ważny.
    Restricted {
        /// Komunikat dostawcy (bez sekretów).
        message: String,
    },
    /// Dostawca rozpoznał żądanie, ale odmawia dostępu z innego powodu niż sam klucz
    /// (kraj lub region, wyłączone API, zapora po drodze). Klucz może być poprawny.
    Refused {
        /// Komunikat dostawcy (bez sekretów).
        message: String,
    },
    /// Klucz poprawny, ale konto u dostawcy nie ma środków albo aktywnych płatności.
    NoCredit {
        /// Komunikat dostawcy (bez sekretów).
        message: String,
    },
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
        matches!(
            self,
            TestOutcome::Ok | TestOutcome::RateLimited | TestOutcome::Restricted { .. }
        )
    }

    /// Czy wynik testu **kandydata** pozwala zapisać go w miejsce dotychczasowego klucza: dostawca
    /// go rozpoznał (także „brak środków”) albo klucza nie da się sprawdzić z góry. Limit zapytań,
    /// błąd sieci i odmowa z innego powodu nic o kluczu nie mówią — działający klucz zostaje.
    pub fn accepts_new_key(&self) -> bool {
        matches!(
            self,
            TestOutcome::Ok
                | TestOutcome::Restricted { .. }
                | TestOutcome::NoCredit { .. }
                | TestOutcome::Unsupported { .. }
        )
    }

    /// Czy po tym wyniku warto pytać o listę modeli (klucz z ograniczeniem dostałby tę samą odmowę).
    pub fn may_list_models(&self) -> bool {
        matches!(
            self,
            TestOutcome::Ok | TestOutcome::RateLimited | TestOutcome::Unsupported { .. }
        )
    }

    /// Stan po ponownym teście tego samego klucza: brak sieci albo limit czasu nie dowodzi
    /// złego klucza, więc konto aktywne, którego poprzedni test potwierdził klucz, zostaje
    /// aktywne (wynik jest w `last_test`). Wcześniej jedna wolna odpowiedź testu w tle wyłączała
    /// konto z Routera do następnego testu, domyślnie na dobę (przegląd 2026-10-21).
    pub fn next_state_after(
        &self,
        current: &AccountState,
        previous: Option<&TestOutcome>,
    ) -> AccountState {
        let inconclusive = matches!(self, TestOutcome::Network { .. } | TestOutcome::Timeout);
        if inconclusive
            && *current == AccountState::Active
            && previous.is_some_and(TestOutcome::key_works)
        {
            return AccountState::Active;
        }
        self.next_state(current)
    }

    /// Stan konta po teście (wyłączone konto zostaje wyłączone; `Unsupported` nie zmienia stanu).
    pub fn next_state(&self, current: &AccountState) -> AccountState {
        if *current == AccountState::Disabled {
            return AccountState::Disabled;
        }
        let error = |kind| AccountState::Error { kind };
        match self {
            TestOutcome::Ok | TestOutcome::RateLimited | TestOutcome::Restricted { .. } => {
                AccountState::Active
            }
            TestOutcome::InvalidKey
            | TestOutcome::NoCredit { .. }
            | TestOutcome::Refused { .. } => error(AccountErrorKind::InvalidKey),
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
            TestOutcome::Restricted { .. } => "restricted",
            TestOutcome::NoCredit { .. } => "no_credit",
            TestOutcome::Refused { .. } => "refused",
            TestOutcome::RateLimited => "rate_limited",
            TestOutcome::Network { .. } => "network",
            TestOutcome::Timeout => "timeout",
            TestOutcome::Unsupported { .. } => "unsupported",
        }
    }
}

/// Co dokładnie odpowiedział dostawca przy teście — do pokazania użytkownikowi obok naszego
/// opisu (sam kod „401” nie mówi, czy klucz jest zły, ograniczony, czy konto nie ma środków).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProbeDetail {
    /// Kod HTTP odpowiedzi.
    pub status: Option<u16>,
    /// Kod błędu dostawcy (np. `invalid_api_key`).
    pub code: Option<String>,
    /// Komunikat dostawcy, skrócony i bez sekretów.
    pub message: String,
}

/// Raport testera połączenia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConnectionReport {
    /// Wynik.
    pub outcome: TestOutcome,
    /// Opóźnienie w ms, jeśli zmierzone.
    pub latency_ms: Option<u64>,
    /// Odpowiedź dostawcy przy niepowodzeniu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<ProbeDetail>,
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
    /// Odpowiedź dostawcy przy niepowodzeniu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<ProbeDetail>,
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
    /// Kiedy zapisano obecny klucz (dodanie konta albo wymiana klucza). Brak — konto sprzed
    /// wprowadzenia pola; najlepszym przybliżeniem jest wtedy `created_at`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_saved_at: Option<DateTime<Utc>>,
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
    /// Kolejki modeli: klasa zadań → modele w kolejności prób (najwyżej
    /// [`crate::MAX_QUEUE`]). Brak wpisu = wybór automatyczny.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub queues: BTreeMap<TaskClass, Vec<ModelId>>,
    /// Konto firmowe: organizacja i projekt OpenAI albo wdrożenia Azure OpenAI.
    #[serde(default, skip_serializing_if = "EnterpriseSettings::is_empty")]
    pub enterprise: EnterpriseSettings,
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
    /// Konto firmowe (puste = zwykłe konto).
    pub enterprise: EnterpriseSettings,
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
