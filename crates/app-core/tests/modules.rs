//! Ustawienia → Moduły na prawdziwym rdzeniu: lista z manifestów, moduły Jądra chronione,
//! wyłączenie wszystkich modułów opcjonalnych działa od następnego uruchomienia (program startuje,
//! rozmowa i sesje działają, moduły są pominięte), ponowne włączenie przywraca je.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, options};

/// Start na profilu; ten sam magazyn sekretów — kolejny start otwiera tę samą zaszyfrowaną bazę.
async fn core(dir: &std::path::Path, secrets: &Arc<app_core::MemorySecretStore>) -> AppCore {
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(app_core::ports::HeadlessShell::default());
    let mut opts = options(Some(provider), shell);
    opts.secrets = Some(secrets.clone());
    AppCore::build(AppPaths::under(dir), opts).await.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn optional_modules_can_be_disabled_until_restart() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(app_core::MemorySecretStore::default());
    let first = core(dir.path(), &secrets).await;
    let list = first.modules_list().await.unwrap();
    assert!(list.modules.len() > 40, "{}", list.modules.len());
    assert!(!list.restart_needed);
    let broker = list
        .modules
        .iter()
        .find(|m| m.id == "safety-broker")
        .unwrap();
    assert!(broker.protected && !broker.can_disable);
    assert!(
        first
            .modules_set_enabled("safety-broker".into(), false)
            .await
            .is_err()
    );
    assert!(
        first
            .modules_set_enabled("sessions".into(), false)
            .await
            .is_err()
    );
    let optional: Vec<String> = list
        .modules
        .iter()
        .filter(|m| m.can_disable)
        .map(|m| m.id.clone())
        .collect();
    assert_eq!(optional.len(), app_modules_count());
    let mut view = list;
    for id in &optional {
        view = first.modules_set_enabled(id.clone(), false).await.unwrap();
    }
    assert!(
        view.restart_needed,
        "zmiana działa po ponownym uruchomieniu"
    );
    drop(first);

    let second = core(dir.path(), &secrets).await;
    let view = second.modules_list().await.unwrap();
    assert!(!view.restart_needed);
    for id in &optional {
        let row = view.modules.iter().find(|m| &m.id == id).unwrap();
        assert!(row.skipped && row.state == "disabled", "{id}: {row:?}");
    }
    // Program działa bez modułów opcjonalnych.
    assert!(second.sessions_list().await.is_ok());
    let _ = second.voice_status().await;
    for id in &optional {
        second.modules_set_enabled(id.clone(), true).await.unwrap();
    }
    drop(second);
    let third = core(dir.path(), &secrets).await;
    let view = third.modules_list().await.unwrap();
    assert!(view.modules.iter().all(|m| !m.skipped && !m.disabled));
}

fn app_modules_count() -> usize {
    5
}
