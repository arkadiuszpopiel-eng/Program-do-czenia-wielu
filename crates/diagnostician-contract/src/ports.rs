//! Porty Diagnosty: środowisko napraw, kontekst (fakty o systemie), Broker (obszar Jądra),
//! otoczenie (czas, zdarzenia, dziennik) i API dla UI.

use async_trait::async_trait;
use core_bus_contract::Event;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::classify::Detection;
use crate::journal::{JournalEntry, RepairId, RepairRecord};
use crate::plan::Proposal;
use crate::report::HealthReport;
use crate::signal::Signal;
use crate::step::RepairStep;

/// Fakty o systemie potrzebne planiście (tylko odczyt).
pub trait RepairContext: Send + Sync {
    /// Bieżący czas (ms).
    fn now_ms(&self) -> u64;
    /// Wartość klucza konfiguracji.
    fn config(&self, key: &str) -> Option<Value>;
    /// Bieżąca rewizja konfiguracji.
    fn current_revision(&self) -> Option<String>;
    /// Ostatnia dobra rewizja (oznaczona po zdrowym starcie).
    fn last_good_revision(&self) -> Option<String>;
    /// Najnowsza kopia zapasowa pliku.
    fn latest_backup(&self, path: &str) -> Option<String>;
    /// Ścieżka kwarantanny dla pliku (unikalna, nieistniejąca).
    fn quarantine_path(&self, path: &str) -> String;
    /// Wolny port w pobliżu.
    fn free_port(&self, near: u16) -> Option<u16>;
    /// Model zastępczy.
    fn fallback_model(&self, model: &str) -> Option<String>;
    /// Katalog zastępczy z prawem zapisu.
    fn fallback_dir(&self, dir: &str) -> Option<String>;
    /// Ścieżka archiwum dla magazynu wpisów.
    fn archive_path(&self, store: &str) -> String;
    /// Pliki do przeniesienia na inny wolumin (pamięć podręczna, stare logi): (skąd, dokąd).
    fn reclaimable(&self, volume: &str) -> Vec<(String, String)>;
    /// Czy ścieżka należy do Jądra (dane Brokera, instalacja, polityki).
    fn is_kernel_path(&self, path: &str) -> bool;
}

/// Wykonawca kroków (poza obszarem Jądra) i sonda po naprawie.
#[async_trait]
pub trait RepairEnv: Send + Sync {
    /// Wykonuje krok; zwraca krok „wykonany” z rzeczywistym stanem sprzed (do cofnięcia).
    /// `SetConfig` działa jak porównaj-i-zamień: bieżąca ≠ `old` → błąd (konflikt).
    async fn apply(&self, step: &RepairStep) -> Result<RepairStep, String>;
    /// Czy awaria ustąpiła.
    async fn verify(&self, detection: &Detection) -> Result<bool, String>;
}

/// Wynik prośby do Brokera.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum KernelOutcome {
    /// Broker wykonał kroki po fizycznym potwierdzeniu.
    Executed {
        /// Kroki wykonane (do cofnięcia).
        receipts: Vec<RepairStep>,
    },
    /// Czeka w Broker-UI.
    Pending {
        /// Bilet.
        ticket: String,
    },
    /// Odmowa (albo Broker niedostępny).
    Denied {
        /// Powód.
        reason: String,
    },
}

/// Broker: jedyny wykonawca napraw w obszarze Jądra (Diagnosta tylko prosi).
#[async_trait]
pub trait KernelApprovals: Send + Sync {
    /// Prośba o wykonanie naprawy (karta w Broker-UI: diff, ryzyko, plan cofnięcia).
    async fn execute(&self, proposal: &Proposal) -> KernelOutcome;
    /// Prośba o cofnięcie wykonanej naprawy.
    async fn undo(&self, proposal: &Proposal, receipts: &[RepairStep]) -> KernelOutcome;
}

/// Brak Brokera: każda naprawa Jądra czeka na człowieka.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoBroker;

#[async_trait]
impl KernelApprovals for NoBroker {
    async fn execute(&self, _: &Proposal) -> KernelOutcome {
        KernelOutcome::Denied {
            reason: "Broker niedostępny — naprawa Jądra wymaga Broker-UI".into(),
        }
    }

    async fn undo(&self, _: &Proposal, _: &[RepairStep]) -> KernelOutcome {
        KernelOutcome::Denied {
            reason: "Broker niedostępny".into(),
        }
    }
}

/// Otoczenie rdzenia.
pub trait DiagHost: Send + Sync + 'static {
    /// Czas (ms).
    fn now_ms(&self) -> u64;
    /// Zdarzenia `diagnostician.*`.
    fn emit(&self, events: Vec<Event>);
    /// Trwały zapis wpisu dziennika napraw (append-only).
    fn append(&self, _entry: &JournalEntry) {}
}

/// Zgoda użytkownika na naprawę.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserConsent {
    /// Skąd (np. `zdrowie-systemu`).
    pub surface: String,
}

/// Błędy Diagnosty.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum DiagError {
    /// Nieznana naprawa.
    #[error("nieznana naprawa {0}")]
    Unknown(String),
    /// Zły stan naprawy.
    #[error("naprawa {id} ma stan `{status}`, oczekiwano `{expected}`")]
    WrongStatus {
        /// Naprawa.
        id: String,
        /// Stan.
        status: String,
        /// Oczekiwany.
        expected: String,
    },
    /// Naprawa obszaru Jądra — zgoda wyłącznie przez Broker-UI.
    #[error("naprawa {0} dotyczy Jądra — zatwierdza ją wyłącznie Broker-UI")]
    KernelAreaRequiresBroker(String),
    /// Limit napraw.
    #[error("limit napraw: {0}")]
    RateLimited(String),
}

/// Wynik skanu.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanOutcome {
    /// Nowe incydenty.
    pub detected: Vec<RepairId>,
    /// Naprawione i zweryfikowane automatycznie.
    pub repaired: Vec<RepairId>,
    /// Czekające na zgodę użytkownika.
    pub awaiting_consent: Vec<RepairId>,
    /// Czekające na Brokera.
    pub kernel_pending: Vec<RepairId>,
    /// Wymagające człowieka.
    pub needs_human: Vec<RepairId>,
    /// Nieudane (cofnięte).
    pub failed: Vec<RepairId>,
}

/// API Diagnosty.
#[async_trait]
pub trait Diagnostician: Send + Sync {
    /// Przyjmuje sygnał (znacznik czasu nadaje Diagnosta).
    async fn ingest(&self, signal: Signal);
    /// Klasyfikacja okna → incydenty → propozycje → naprawy wg autonomii.
    async fn scan(&self) -> ScanOutcome;
    /// Zgoda użytkownika na naprawę (nie dotyczy obszaru Jądra).
    async fn approve(&self, id: RepairId, consent: UserConsent) -> Result<RepairRecord, DiagError>;
    /// Odrzucenie propozycji.
    async fn reject(&self, id: RepairId) -> Result<RepairRecord, DiagError>;
    /// Cofnięcie wykonanej naprawy (operacje odwrotne w odwrotnej kolejności).
    async fn undo(&self, id: RepairId) -> Result<RepairRecord, DiagError>;
    /// Raport „Zdrowie systemu”.
    fn report(&self) -> HealthReport;
    /// Naprawy (wszystkie).
    fn repairs(&self) -> Vec<RepairRecord>;
    /// Dziennik napraw (append-only).
    fn journal(&self) -> Vec<JournalEntry>;
}
