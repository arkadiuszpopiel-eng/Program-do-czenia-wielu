//! STOP WSZYSTKIEGO a potwierdzenie Brokera (audyt 2026-10-14, A-03): gdy Broker nie potwierdził
//! zatrzymania (nieosiągalny, audyt niezapisany, proces nie dał się zabić), Alfa nie mówi
//! „zatrzymano”, tylko pokazuje trwały błąd o zatrzymaniu niepełnym. Gdy potwierdził — zwykły
//! komunikat.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use app_core::dto::{AlfaEvent, AutonomyLevel, BrokerIntentResult, BrokerStatusView, ToastKind};
use app_core::ports::{BrokerPort, HeadlessShell, KillOrigin};
use app_core::{AppCore, AppError, AppPaths};
use async_trait::async_trait;
use common::{options, until};

/// Broker, którego kill-switch zwraca zadany wynik.
struct Broker(Option<String>);

#[async_trait]
impl BrokerPort for Broker {
    async fn request_level(
        &self,
        _level: AutonomyLevel,
        _session: Option<&sessions_contract::SessionId>,
    ) -> Result<BrokerIntentResult, AppError> {
        Err(AppError::invalid("nieużywane w teście"))
    }
    async fn open_approval(&self, _approval: &str) -> Result<BrokerIntentResult, AppError> {
        Err(AppError::invalid("nieużywane w teście"))
    }
    async fn run_code(
        &self,
        _session: &sessions_contract::SessionId,
        _lang: Option<&str>,
        _code: &str,
    ) -> Result<BrokerIntentResult, AppError> {
        Err(AppError::invalid("nieużywane w teście"))
    }
    async fn undo_step(
        &self,
        _session: &sessions_contract::SessionId,
        _step: u64,
    ) -> Result<String, AppError> {
        Err(AppError::invalid("nieużywane w teście"))
    }
    async fn kill_all(&self, _origin: KillOrigin) -> Result<Option<String>, AppError> {
        Ok(self.0.clone())
    }
    fn status(&self) -> BrokerStatusView {
        BrokerStatusView::unavailable()
    }
}

async fn stop_with(gap: Option<String>) -> (ToastKind, String) {
    let dir = tempfile::tempdir().unwrap();
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.broker = Some(Arc::new(Broker(gap)));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    let mut rx = core.subscribe_events();
    let stop = {
        let core = core.clone();
        tokio::spawn(async move { core.system_kill_all(KillOrigin::Hotkey).await })
    };
    let events = until(&mut rx, |e| matches!(e, AlfaEvent::Toast { .. })).await;
    stop.await.unwrap();
    let toast = events
        .into_iter()
        .rev()
        .find_map(|e| match e {
            AlfaEvent::Toast { kind, message } => Some((kind, message.pl)),
            _ => None,
        })
        .unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    toast
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confirmed_stop_says_stopped() {
    let (kind, text) = stop_with(None).await;
    assert_eq!(kind, ToastKind::Warning);
    assert_eq!(text, "STOP WSZYSTKIEGO: zatrzymano pracę agentek.");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unconfirmed_stop_is_reported_as_incomplete() {
    let (kind, text) = stop_with(Some("Sędzia Cieni nie potwierdził zatrzymania".to_owned())).await;
    assert_eq!(kind, ToastKind::Error);
    assert!(text.contains("NIEPEŁNE"), "{text}");
    assert!(
        text.contains("Sędzia Cieni nie potwierdził zatrzymania"),
        "{text}"
    );
    assert!(!text.contains("zatrzymano pracę"), "{text}");
}
