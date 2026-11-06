//! `permissions_request_max_access` (plan 50 §8.1): włączenie „Max dostęp” to karta w oknie
//! Sędziego Cieni (bez okna — odmowa), stan widać w `permissions_get` dopiero po zgodzie,
//! wyłączenie działa od razu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};

use app_core::dto::{BrokerIntentStatus, MaxAccess};
use app_core::ports::{ApprovalWindow, HeadlessShell};
use app_core::{AppCore, AppError, AppPaths};
use common::*;

#[derive(Default)]
struct Window(Mutex<Vec<String>>);

impl ApprovalWindow for Window {
    fn present(&self, approval: &str) -> Result<(), AppError> {
        self.0.lock().unwrap().push(approval.to_owned());
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn max_access_goes_through_the_judge_window() {
    let h = harness().await;
    assert_eq!(h.core.permissions_get(None).await.unwrap().max_access, None);
    let denied = h
        .core
        .permissions_request_max_access(Some(MaxAccess::Local))
        .await
        .unwrap_err();
    assert_eq!(denied.message, app_core::ports::NEEDS_BROKER_WINDOW);
    // Wyłączanie wyłączonego — nic do zrobienia, od razu.
    let off = h.core.permissions_request_max_access(None).await.unwrap();
    assert_eq!(off.status, BrokerIntentStatus::Applied);

    let dir = tempfile::tempdir().unwrap();
    let window = Arc::new(Window::default());
    let mut opts = options(None, Arc::new(HeadlessShell::default()));
    opts.approval_window = Some(window.clone());
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    for scope in [MaxAccess::Local, MaxAccess::Cloud, MaxAccess::All] {
        let opened = core
            .permissions_request_max_access(Some(scope))
            .await
            .unwrap();
        assert_eq!(opened.status, BrokerIntentStatus::OpenedBroker);
        assert_eq!(
            window.0.lock().unwrap().last(),
            Some(&opened.request_id),
            "karta w oknie Sędziego"
        );
        assert_eq!(
            core.permissions_get(None).await.unwrap().max_access,
            None,
            "bez zgody właściciela wyłączony"
        );
    }
}
