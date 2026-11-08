//! Porty i API Ulepszacza.

use async_trait::async_trait;
use core_bus_contract::Event;
use evals_contract::EvalError;
use serde::{Deserialize, Serialize};

use crate::guard::Violation;
use crate::policy::ImproverPolicy;
use crate::proposal::{
    BlockedAttempt, CandidateSet, IssueDraft, MetricsSnapshot, Proposal, ProposalId, RunConditions,
    UserApproval,
};

/// Błędy Ulepszacza.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "error", content = "detail", rename_all = "snake_case")]
pub enum ImproverError {
    /// Nie teraz (bateria, gra, użytkownik aktywny).
    #[error("Ulepszacz wstrzymany: {0}")]
    NotNow(String),
    /// Naruszenie granic — zestaw odrzucony w całości.
    #[error("odrzucone przez strażnika: {0}")]
    Guard(Violation),
    /// Nieznana propozycja.
    #[error("nieznana propozycja {0}")]
    UnknownProposal(String),
    /// Zły etap.
    #[error("propozycja {id} jest na etapie `{stage}`, oczekiwano `{expected}`")]
    WrongStage {
        /// Propozycja.
        id: String,
        /// Bieżący etap.
        stage: String,
        /// Oczekiwany.
        expected: String,
    },
    /// Zatwierdzenie nieważne (inny diff, brak podpisu, inna propozycja).
    #[error("zatwierdzenie nieważne: {0}")]
    ApprovalInvalid(String),
    /// Limit częstotliwości.
    #[error("limit: {0}")]
    RateLimited(String),
    /// Klucz w okresie wychładzania po rollbacku.
    #[error("klucz `{0}` w okresie wychładzania po rollbacku")]
    Cooldown(String),
    /// Bramka ewaluacyjna.
    #[error("bramka ewaluacyjna: {0}")]
    Gate(EvalError),
    /// Zapis/odczyt konfiguracji.
    #[error("konfiguracja: {0}")]
    Config(String),
    /// Polityka niepoprawna.
    #[error("polityka Ulepszacza: {0}")]
    Policy(String),
}

/// Źródło propozycji (np. retrospektywa modelem lokalnym) — wynik jest niezaufany.
#[async_trait]
pub trait Proposer: Send + Sync {
    /// Nazwa (do pola `source`).
    fn name(&self) -> String;
    /// Kandydaci na zmiany z migawki.
    async fn propose(&self, snapshot: &MetricsSnapshot) -> Result<Vec<CandidateSet>, String>;
}

/// Weryfikacja zatwierdzenia (kompozycja: podpis TPM / Windows Hello dla R1–R2).
pub trait ApprovalVerifier: Send + Sync {
    /// Czy zatwierdzenie jest ważne dla tej propozycji.
    fn verify(&self, approval: &UserApproval, proposal: &Proposal) -> bool;
}

/// Otoczenie rdzenia.
pub trait ImproverHost: Send + Sync + 'static {
    /// Czas (ms).
    fn now_ms(&self) -> u64;
    /// Zdarzenia (Audyt „samo-zmiany” przekazuje kompozycja do Brokera).
    fn emit(&self, events: Vec<Event>);
    /// Trwały zapis kolejki propozycji.
    fn persist(&self, _proposals: &[Proposal]) {}
}

/// API Ulepszacza (panel „Zdrowie systemu”, harmonogram bezczynności).
#[async_trait]
pub trait Improver: Send + Sync {
    /// R0: obserwacja (reguły + porty `Proposer`) → propozycje po strażniku.
    async fn observe(
        &self,
        snapshot: &MetricsSnapshot,
        conditions: RunConditions,
    ) -> Result<Vec<Proposal>, ImproverError>;
    /// Propozycja z zewnątrz — ta sama ścieżka strażnika.
    async fn submit(&self, candidate: CandidateSet) -> Result<Proposal, ImproverError>;
    /// R1: piaskownica (podział `test`, przed/po) i holdout (bramka Jądra); R0 bezpieczne → wdrożenie.
    async fn evaluate(&self, id: ProposalId) -> Result<Proposal, ImproverError>;
    /// R2: wdrożenie po zatwierdzeniu użytkownika.
    async fn approve(&self, approval: UserApproval) -> Result<Proposal, ImproverError>;
    /// Odrzucenie.
    async fn reject(&self, id: ProposalId) -> Result<Proposal, ImproverError>;
    /// Nadzór po wdrożeniu; regresja → automatyczny rollback (zwraca cofnięte).
    async fn monitor(&self, snapshot: &MetricsSnapshot) -> Result<Vec<Proposal>, ImproverError>;
    /// Ręczny rollback.
    async fn rollback(&self, id: ProposalId) -> Result<Proposal, ImproverError>;
    /// Wszystkie propozycje.
    fn proposals(&self) -> Vec<Proposal>;
    /// Zablokowane próby.
    fn blocked(&self) -> Vec<BlockedAttempt>;
    /// Szkice zgłoszeń R3.
    fn issue_drafts(&self) -> Vec<IssueDraft>;
    /// Polityka (tylko odczyt).
    fn policy(&self) -> ImproverPolicy;
}
