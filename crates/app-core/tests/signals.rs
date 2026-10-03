//! Sygnały systemowe i obserwacja katalogów w kompozycji (atrapy `platform-fake`): z monitorem
//! sygnałów cykl Ulepszacza działa w bezczynności (bez niego — tylko „Przeanalizuj teraz");
//! nowy plik w obserwowanym katalogu uruchamia wyzwalacz plikowy (pompa `DirWatchPort` →
//! `TriggersModule::file_created`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use app_core::dto::{AlfaEvent, TriggerDraft, TriggerKindView};
use app_core::{AppCore, AppPaths};
use common::*;
use platform_fake::{FakeDirWatch, FakeSignals};

/// Katalog wyzwalacza — ścieżka bezwzględna dla systemu: na Windows `/home/…` nie ma litery dysku,
/// więc obserwacja odrzuca ją jako `InvalidPath` i wyzwalacz nie dostaje zdarzeń.
fn downloads() -> PathBuf {
    PathBuf::from(if cfg!(windows) {
        r"C:\Users\ala\Pobrane"
    } else {
        "/home/ala/Pobrane"
    })
}

async fn core_with(
    signals: Option<Arc<FakeSignals>>,
    watch: Option<Arc<FakeDirWatch>>,
) -> (AppCore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let shell = Arc::new(app_core::ports::HeadlessShell::default());
    let mut opts = options(None, shell);
    opts.signals = signals.map(|s| s as Arc<dyn platform_contract::SystemSignalsPort>);
    opts.dir_watch = watch.map(|w| w as Arc<dyn platform_contract::DirWatchPort>);
    let core = AppCore::build(AppPaths::under(dir.path()), opts)
        .await
        .unwrap();
    (core, dir)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_cycle_only_with_a_signals_monitor() {
    let (core, _dir) = core_with(Some(Arc::new(FakeSignals::default())), None).await;
    assert!(core.improver_list().await.unwrap().idle_cycle);
    // Bez monitora w opcjach rdzeń bierze monitor systemu (`AppOptions::signals = None`): na Windows
    // się uruchamia, poza Windows nie — wtedy cykl tylko ręcznie („Przeanalizuj teraz").
    let native = platform_windows_sys_impl::WinSignals::default()
        .start()
        .is_ok();
    assert!(
        cfg!(windows) || !native,
        "poza Windows monitor systemu się nie uruchamia"
    );
    let (plain, _dir2) = core_with(None, None).await;
    assert_eq!(
        plain.improver_list().await.unwrap().idle_cycle,
        native,
        "cykl w bezczynności tylko z monitorem (poza Windows — tylko ręcznie)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn new_file_in_watched_dir_fires_file_trigger() {
    let watch = Arc::new(FakeDirWatch::default());
    let (core, _dir) = core_with(None, Some(watch.clone())).await;
    let mut rx = core.subscribe_events();
    let trigger = core
        .triggers_create(TriggerDraft {
            name: "Faktury".into(),
            kind: TriggerKindView::FileInDir {
                dir: downloads().to_string_lossy().into_owned(),
                pattern: Some("*.pdf".into()),
            },
            title: "Opisz fakturę".into(),
            goal: "Opisz nową fakturę w Pobranych.".into(),
            agent: Some("beta".into()),
            bridge: None,
            respect_dnd: false,
        })
        .await
        .unwrap();
    for _ in 0..100 {
        if !platform_contract::DirWatchPort::watches(&*watch).is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        platform_contract::DirWatchPort::watches(&*watch).len(),
        1,
        "katalog wyzwalacza obserwowany"
    );
    watch.write(downloads().join("notatka.txt"), 10);
    watch.write(downloads().join("faktura-04.pdf"), 2_048);
    let fired = until(
        &mut rx,
        |e| matches!(e, AlfaEvent::TriggerFired { run } if run.trigger_id == trigger.id),
    )
    .await;
    let run = fired
        .iter()
        .find_map(|e| match e {
            AlfaEvent::TriggerFired { run } if run.trigger_id == trigger.id => Some(run.clone()),
            _ => None,
        })
        .unwrap();
    assert!(run.cause.contains("faktura-04.pdf"), "{run:?}");
    let log = core.triggers_log(Some(trigger.id.clone())).await.unwrap();
    assert_eq!(log.len(), 1, "tylko plik pasujący do wzorca: {log:?}");
}
