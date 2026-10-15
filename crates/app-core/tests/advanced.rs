//! Ustawienia → Zaawansowane na prawdziwym rdzeniu: surowy edytor plików konfiguracji —
//! poprawny zapis zmienia ustawienie, a składnia, zły typ i klucz Jądra są odrzucane
//! z przywróceniem poprzedniej treści pliku.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use app_core::dto::SettingValue;
use app_core::{AppCore, AppPaths};
use common::{ScriptedProvider, options};

async fn core(dir: &std::path::Path) -> AppCore {
    let provider = Arc::new(ScriptedProvider::new(Duration::from_millis(1)));
    let shell = Arc::new(app_core::ports::HeadlessShell::default());
    AppCore::build(AppPaths::under(dir), options(Some(provider), shell))
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_editor_validates_and_restores() {
    let dir = tempfile::tempdir().unwrap();
    let core = core(dir.path()).await;
    let raw = core.config_raw_get().await.unwrap();
    assert!(raw.shared_path.ends_with("shared.toml"));
    assert_eq!(
        core.config_raw_check("[ui]\ntheme = \"dark\"\n".into())
            .await
            .unwrap(),
        1
    );
    let bad = core
        .config_raw_check("[ui]\ntheme = \n".into())
        .await
        .unwrap_err();
    assert!(bad.message.contains("wiersz 2"), "{}", bad.message);

    let saved = core
        .config_raw_save("shared".into(), "[ui]\ntheme = \"dark\"\n".into())
        .await
        .unwrap();
    assert!(saved.shared.contains("theme = \"dark\""));
    let values = core.settings_values().await.unwrap();
    assert_eq!(
        values.get("ui.theme"),
        Some(&SettingValue::Text("dark".into()))
    );

    for (text, what) in [
        ("[ui]\ntheme = 5\n", "zły typ"),
        ("[kernel]\nautonomy_max = \"L4\"\n", "klucz Jądra"),
        ("[ui\n", "składnia"),
    ] {
        let err = core.config_raw_save("shared".into(), text.into()).await;
        assert!(err.is_err(), "{what}");
        let now = core.config_raw_get().await.unwrap();
        assert!(
            now.shared.contains("theme = \"dark\""),
            "{what}: plik przywrócony"
        );
    }
    let values = core.settings_values().await.unwrap();
    assert_eq!(
        values.get("ui.theme"),
        Some(&SettingValue::Text("dark".into()))
    );
    assert!(
        core.config_raw_save("kernel".into(), String::new())
            .await
            .is_err()
    );
}
