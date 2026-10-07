//! Trait [`AgentBackend`] (PLAN §5.1) i port [`Workspace`] (izolowany katalog roboczy, §8.5).

use std::path::PathBuf;
use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::approval::{ApprovalDecision, PermissionRequestId};
use crate::error::BackendError;
use crate::event::AgentEventEnvelope;
use crate::task::{SessionRef, TaskHandle, TaskId, TaskSpec, WorkdirSpec};

/// Strumień zdarzeń zadania: odtwarza zdarzenia od początku, potem na żywo; kończy się po
/// zdarzeniu końcowym.
pub type AgentEventStream = Pin<Box<dyn Stream<Item = AgentEventEnvelope> + Send>>;

/// Backend agentowy (zadania → strumień zdarzeń).
#[async_trait]
pub trait AgentBackend: Send + Sync {
    /// Przyjmuje zadanie. Odmowa (bez uruchamiania procesu) gdy: pochodzenie niedozwolone,
    /// trasa wyłączona w `compliance`, wersja CLI nieprzypięta.
    async fn submit_task(&self, spec: TaskSpec) -> Result<TaskHandle, BackendError>;

    /// Zdarzenia zadania.
    fn events(&self, task: &TaskId) -> Result<AgentEventStream, BackendError>;

    /// Decyzja w sprawie prośby o uprawnienie.
    async fn approve(
        &self,
        request: &PermissionRequestId,
        decision: ApprovalDecision,
    ) -> Result<(), BackendError>;

    /// Wiadomość sterująca (między atomowymi krokami, §9.6).
    async fn steer(&self, task: &TaskId, message: String) -> Result<(), BackendError>;

    /// Anuluje zadanie: zabija drzewo procesów (≤ 2 s), odrzuca oczekujące prośby.
    async fn cancel(&self, task: &TaskId) -> Result<(), BackendError>;

    /// Wznawia sesję CLI w jej katalogu roboczym (te same reguły co `submit_task`).
    async fn resume(
        &self,
        session: SessionRef,
        mut spec: TaskSpec,
    ) -> Result<TaskHandle, BackendError> {
        if session.bridge != spec.bridge {
            return Err(BackendError::InvalidSpec(
                "most sesji różni się od mostu zadania".into(),
            ));
        }
        spec.session = Some(session);
        self.submit_task(spec).await
    }
}

/// Rodzaj przygotowanego katalogu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkdirKind {
    /// `git worktree` (odłączony HEAD).
    GitWorktree,
    /// Kopia katalogu.
    Copy,
    /// Katalog sesji wznawianej (przygotowany wcześniej).
    Resumed,
}

/// Przygotowany katalog roboczy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PreparedWorkdir {
    /// Ścieżka, w której pracuje CLI (zawsze pod katalogiem roboczym Alfy).
    pub path: PathBuf,
    /// Źródło (katalog użytkownika).
    pub source: PathBuf,
    /// Rodzaj.
    pub kind: WorkdirKind,
}

/// Port izolowanego katalogu roboczego.
#[async_trait]
pub trait Workspace: Send + Sync {
    /// Tworzy worktree/kopię dla zadania; nigdy nie zwraca katalogu źródłowego.
    async fn prepare(
        &self,
        task: &TaskId,
        spec: &WorkdirSpec,
    ) -> Result<PreparedWorkdir, BackendError>;

    /// Sprawdza katalog wznawianej sesji (musi leżeć pod katalogiem roboczym Alfy).
    async fn reuse(&self, session: &SessionRef) -> Result<PreparedWorkdir, BackendError>;

    /// Zwalnia katalog (`keep = true` — zostaje do przeglądu/scalenia przez użytkownika).
    async fn release(&self, prepared: &PreparedWorkdir, keep: bool) -> Result<(), BackendError>;
}
