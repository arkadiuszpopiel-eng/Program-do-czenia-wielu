//! Wspólne elementy przebiegu procesu CLI: kontekst zadania, końcówka stderr, zabijanie drzewa,
//! budżet czasu ściennego, zakończenie zadania.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agent_backends_contract::{AgentEvent, BackendError, BridgeKind, TaskBudget};
use providers_contract::CancellationToken;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Child;
use tokio::sync::mpsc;

use crate::approvals::Approvals;
use crate::config::BridgeConfig;
use crate::log::TaskLog;
use crate::process::TreeKiller;

/// Ile bajtów końcówki stderr zachować (diagnostyka błędów CLI).
const STDERR_TAIL_BYTES: usize = 4096;

/// Kontekst przebiegu.
pub struct RunCtx {
    /// Dziennik zdarzeń.
    pub log: Arc<TaskLog>,
    /// Hub zatwierdzeń.
    pub approvals: Arc<Approvals>,
    /// Anulowanie.
    pub cancel: CancellationToken,
    /// Wiadomości sterujące.
    pub steer: mpsc::UnboundedReceiver<String>,
    /// Zabójca drzew procesów.
    pub killer: Arc<dyn TreeKiller>,
    /// Konfiguracja.
    pub config: Arc<BridgeConfig>,
    /// Katalog roboczy.
    pub workdir: PathBuf,
    /// Budżet.
    pub budget: TaskBudget,
    /// Most.
    pub bridge: BridgeKind,
}

/// Jak zakończyła się pętla przebiegu.
pub enum Outcome {
    /// Anulowanie.
    Cancelled,
    /// Przekroczony budżet.
    Budget(String),
    /// Koniec stdout (proces kończy się sam).
    Eof,
    /// Gotowe zdarzenie końcowe (Codex: koniec tury) — proces trzeba zamknąć.
    Finished(AgentEvent),
}

/// Zbiera końcówkę stderr (bez interpretacji; nie trafia do logów w całości).
pub fn stderr_tail<R: AsyncRead + Unpin + Send + 'static>(
    mut stderr: R,
) -> tokio::task::JoinHandle<String> {
    tokio::spawn(async move {
        let mut tail: Vec<u8> = Vec::new();
        let mut buf = [0u8; 4096];
        while let Ok(n) = stderr.read(&mut buf).await {
            if n == 0 {
                break;
            }
            tail.extend_from_slice(&buf[..n]);
            if tail.len() > STDERR_TAIL_BYTES {
                let cut = tail.len() - STDERR_TAIL_BYTES;
                tail.drain(..cut);
            }
        }
        String::from_utf8_lossy(&tail).into_owned()
    })
}

/// Sen do końca budżetu czasu ściennego (albo „nigdy”).
pub async fn wall_clock(budget: &TaskBudget) {
    match budget.wall_clock_ms {
        Some(ms) => tokio::time::sleep(Duration::from_millis(ms)).await,
        None => std::future::pending::<()>().await,
    }
}

/// Zabija drzewo procesów i czeka na zakończenie procesu (najwyżej `grace`).
pub async fn kill(child: &mut Child, killer: &dyn TreeKiller, grace: Duration) {
    if let Some(pid) = child.id()
        && let Err(e) = killer.kill_tree(pid)
    {
        tracing::warn!(blad = %e, "nie udało się zabić drzewa procesów CLI");
    }
    let _ = child.start_kill();
    let _ = tokio::time::timeout(grace, child.wait()).await;
}

/// Zamyka proces po zakończonym zadaniu: czeka `grace` na samodzielne wyjście, potem zabija.
pub async fn shutdown(child: &mut Child, killer: &dyn TreeKiller, grace: Duration) {
    if tokio::time::timeout(grace, child.wait()).await.is_err() {
        kill(child, killer, grace).await;
    }
}

/// Czeka na wyjście procesu po końcu stdout; zwraca kod (albo `None` — sygnał/zabity).
pub async fn exit_code(child: &mut Child, killer: &dyn TreeKiller, grace: Duration) -> Option<i32> {
    match tokio::time::timeout(grace, child.wait()).await {
        Ok(Ok(status)) => status.code(),
        _ => {
            kill(child, killer, grace).await;
            None
        }
    }
}

/// Zdarzenie końcowe dla anulowania/budżetu.
pub fn abort_event(outcome: &Outcome) -> Option<AgentEvent> {
    match outcome {
        Outcome::Cancelled => Some(AgentEvent::Error {
            error: BackendError::Cancelled,
        }),
        Outcome::Budget(what) => Some(AgentEvent::Error {
            error: BackendError::BudgetExceeded(what.clone()),
        }),
        _ => None,
    }
}

/// Czy zdarzenie `Usage` przekracza budżet kosztu.
pub fn over_cost(budget: &TaskBudget, event: &AgentEvent) -> bool {
    matches!((budget.max_cost_micro_usd, event),
        (Some(max), AgentEvent::Usage { cost_micro_usd: Some(cost), .. }) if *cost > max)
}
