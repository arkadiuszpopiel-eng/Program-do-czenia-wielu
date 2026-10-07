//! Kopie zapasowe: katalog tylko z natywnego dialogu, rotacja, harmonogram (odstęp, bateria),
//! hasło w sejfie (szyfrowanie, sesje prywatne), sekrety nigdy w paczce oraz **test przywracania**
//! (kopia → świeża instalacja → import → te same sesje, konfiguracja i artefakty).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use accounts_hub_contract::{SecretName, SecretStore, SecretString};
use accounts_hub_fake::MemorySecretStore;
use app_api::dto::BackupConfig;
use app_files::ArtifactDocuments;
use artifacts_contract::{Artifacts, Origin};
use artifacts_fake::FakeArtifacts;
use common::{Env, transfer};
use sessions_contract::{NewTurn, SessionCatalog, SessionHistory, Sessions};
use sessions_fake::FakeSessions;
use transfer_contract::{DocumentStore, ImportOptions, Transfer};
use transfer_impl::{DirDocumentStore, DirFilter};

async fn configured(env: &Env, keep: u32) -> std::path::PathBuf {
    let dir = env.dir.path().join("Kopie");
    std::fs::create_dir_all(&dir).unwrap();
    env.shell.answer_dialog(Some(dir.clone()));
    let view = env.app.backups_choose_dir().await.unwrap();
    assert_eq!(
        view.config.dir.as_deref(),
        Some(dir.to_string_lossy().as_ref())
    );
    env.app
        .backups_configure(BackupConfig {
            enabled: true,
            dir: Some("/ignorowane/z/UI".into()),
            interval_hours: 24,
            keep,
            include_artifacts: true,
            include_logs: false,
            skip_on_battery: true,
        })
        .unwrap();
    dir
}

#[tokio::test]
async fn dir_comes_only_from_dialog_and_protected_dirs_are_refused() {
    let env = Env::new();
    let view = env.app.backups_status();
    assert!(!view.config.enabled && view.config.dir.is_none());
    let v = env
        .app
        .backups_configure(BackupConfig {
            enabled: true,
            dir: Some("/tmp/obcy".into()),
            interval_hours: 0,
            keep: 0,
            include_artifacts: false,
            include_logs: false,
            skip_on_battery: true,
        })
        .unwrap();
    assert!(
        !v.config.enabled,
        "bez katalogu z dialogu kopie są wyłączone"
    );
    assert_eq!(v.config.dir, None);
    assert_eq!((v.config.interval_hours, v.config.keep), (1, 1));
    std::fs::create_dir_all(&env.paths.local).unwrap();
    env.shell.answer_dialog(Some(env.paths.local.clone()));
    assert!(env.app.backups_choose_dir().await.is_err());
    env.shell.answer_dialog(None);
    assert_eq!(env.app.backups_choose_dir().await.unwrap().config.dir, None);
    assert!(env.app.backups_run_now().await.is_err(), "bez katalogu");
}

#[tokio::test]
async fn rotation_keeps_newest_and_schedule_respects_interval_and_battery() {
    let env = Env::new();
    let s = env.session("Kopia");
    env.sessions
        .append_turn(&s, None, NewTurn::user("treść"))
        .unwrap();
    let dir = configured(&env, 2).await;
    for _ in 0..3 {
        env.app.backups_run_now().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let view = env.app.backups_status();
    assert_eq!(view.entries.len(), 2, "rotacja: 2 najnowsze");
    assert!(view.entries[0].created_at > view.entries[1].created_at);
    assert!(view.last_run.is_some() && view.last_error.is_none());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
    let service = env.app.backups().clone();
    let now = chrono::Utc::now();
    assert!(!service.tick(now).await, "kopia przed chwilą — nie należna");
    let later = now + chrono::Duration::hours(25);
    env.signals.on_battery(true);
    assert!(!service.tick(later).await, "na baterii — nie");
    env.signals.on_battery(false);
    assert!(service.tick(later).await);
    assert_eq!(env.app.backups_status().entries.len(), 2);
}

#[tokio::test]
async fn secrets_never_enter_backups_and_password_encrypts() {
    let env = Env::new();
    let s = env.session("Sekrety");
    let key = "sk-ant-api03-TAJNYKLUCZ1234567890abcdefghij";
    env.secrets
        .put(
            &SecretName::new("anthropic/default").unwrap(),
            &SecretString::from(key),
        )
        .unwrap();
    env.sessions
        .append_turn(&s, None, NewTurn::user(format!("mój klucz to {key}")))
        .unwrap();
    std::fs::create_dir_all(&env.paths.config).unwrap();
    std::fs::write(
        env.paths.config.join("ui.toml"),
        format!("theme = \"dark\"\nkey = \"{key}\"\n"),
    )
    .unwrap();
    configured(&env, 5).await;
    env.app.backups_run_now().await.unwrap();
    let plain = env.app.backups_status().entries[0].clone();
    let bytes = std::fs::read(&plain.path).unwrap();
    assert!(
        !bytes.windows(key.len()).any(|w| w == key.as_bytes()),
        "sekret nie trafia do kopii"
    );
    assert!(
        env.app
            .backups_set_password(Some(serde_json::from_str("\"krótkie\"").unwrap()))
            .is_err()
    );
    let view = env
        .app
        .backups_set_password(Some(
            serde_json::from_str("\"długie hasło kopii 2026\"").unwrap(),
        ))
        .unwrap();
    assert!(view.password_set);
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    env.app.backups_run_now().await.unwrap();
    let newest = env.app.backups_status().entries[0].clone();
    let check = env.app.backups_verify(&newest.file).await.unwrap();
    assert!(check.ok && check.encrypted, "{check:?}");
    assert_eq!(check.sessions, 1);
    assert!(env.app.backups_verify("../../etc/passwd").await.is_err());
    assert!(!env.app.backups_set_password(None).unwrap().password_set);
    let locked = env.app.backups_verify(&newest.file).await.unwrap();
    assert!(!locked.ok && locked.message.is_some());
}

#[tokio::test]
async fn restore_test_backup_imports_into_fresh_installation() {
    let env = Env::new();
    let s = env.session("Przywracanie");
    let t = env
        .sessions
        .append_turn(&s, None, NewTurn::user("pierwsza"))
        .unwrap();
    env.sessions
        .append_turn(&s, Some(t.id), NewTurn::assistant("alfa", "druga"))
        .unwrap();
    std::fs::create_dir_all(&env.paths.config).unwrap();
    std::fs::write(env.paths.config.join("ui.toml"), "theme = \"dark\"\n").unwrap();
    let out = env.paths.workdirs().join("Przywracanie").join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("wynik.txt"), "artefakt").unwrap();
    env.artifacts
        .register(&s, &out.join("wynik.txt"), Origin::User, None)
        .unwrap();
    configured(&env, 3).await;
    env.app.backups_run_now().await.unwrap();
    let entry = env.app.backups_status().entries[0].clone();
    assert!(env.app.backups_verify(&entry.file).await.unwrap().ok);

    // Świeża instalacja: puste sesje, konfiguracja i artefakty.
    let fresh = tempfile::tempdir().unwrap();
    let sessions = Arc::new(FakeSessions::new());
    let secrets = Arc::new(MemorySecretStore::new());
    let config = Arc::new(DirDocumentStore::new(
        fresh.path().join("config"),
        DirFilter::flat(&["toml"]),
    ));
    let artifacts = Arc::new(FakeArtifacts::new(fresh.path().join("user")));
    let docs = ArtifactDocuments::new(
        fresh.path().join("user").join("Sesje").join("Import"),
        1 << 20,
    );
    docs.bind(
        artifacts.clone() as Arc<dyn Artifacts>,
        sessions.clone() as Arc<dyn Sessions>,
    );
    let restored = transfer(&fresh, &sessions, &secrets, &config, &docs, "snap");
    let report = restored
        .import(std::path::Path::new(&entry.path), &ImportOptions::default())
        .unwrap();
    assert_eq!(report.failed, 0, "{report:?}");
    let meta = sessions.session(&s).unwrap();
    assert_eq!(meta.title, "Przywracanie");
    let leaf = sessions.active_leaf(&s).unwrap().unwrap();
    let texts: Vec<String> = sessions
        .branch_projection(&s, leaf)
        .unwrap()
        .into_iter()
        .map(|t| t.content.text)
        .collect();
    assert_eq!(texts, ["pierwsza", "druga"]);
    assert_eq!(
        config.read("ui.toml").unwrap().as_deref(),
        Some("theme = \"dark\"\n".as_bytes())
    );
    let names = docs.list().unwrap();
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(names[0].ends_with("/wynik.txt"));
    assert_eq!(
        docs.read(&names[0]).unwrap().as_deref(),
        Some(&b"artefakt"[..])
    );
    let snapshots = restored.snapshots().unwrap();
    assert_eq!(snapshots.len(), 1, "snapshot przed importem");
    let rollback = restored.rollback(&snapshots[0].id).unwrap();
    assert!(rollback.removed + rollback.restored > 0);
    assert!(
        config.read("ui.toml").unwrap().is_none(),
        "rollback cofa import"
    );
}
