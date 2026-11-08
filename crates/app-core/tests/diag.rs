//! Diagnostyka komputera w rdzeniu (`diag_*`) na atrapie sondy: stan, zapis, testy z kluczy,
//! raport w folderze `Diagnostyka`, błędne klucze.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use app_core::dto::Verdict;
use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, options};

async fn core() -> (AppCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(app_core::ports::HeadlessShell::default());
    let mut opts = options(Some(provider), shell);
    opts.diag_probe = Some(Arc::new(pc_diag_fake::FakeProbe::default()));
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    (core, dir)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diag_commands_through_core() {
    let (core, dir) = core().await;
    let empty = core.diag_state().await.unwrap();
    assert!(empty.latest.is_none() && empty.running.is_none());
    let v = core.diag_snapshot().await.unwrap();
    assert_eq!(v.integrity.unwrap().verdict, Verdict::Warn);
    assert!(v.available.contains(&"disk:C:".to_owned()));
    assert!(core.diag_run(vec!["format_c".into()]).await.is_err());
    let run = core
        .diag_run(vec!["integrity".into(), "cpu_single".into()])
        .await
        .unwrap();
    assert_eq!(run.results.len(), 2);
    assert_eq!(core.diag_runs(5).await.unwrap().len(), 1);
    core.diag_cancel().await.unwrap();
    let path = core.diag_export().await.unwrap();
    assert!(path.contains("Diagnostyka"));
    assert!(std::path::Path::new(&path).starts_with(dir.path()));
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("Diagnostyka komputera")
    );
}
