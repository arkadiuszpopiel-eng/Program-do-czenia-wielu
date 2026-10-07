//! Eksport → import `.alfa` przez komendy (moduł `transfer`): dialogi powłoki, podgląd
//! (dry-run), tryb, snapshot i rollback, paczka szyfrowana hasłem; sekretów nie eksportuje się nigdy.
//! Plik importu tylko jednorazowym uchwytem (dialog, wynik podglądu, „Przywróć…” z listy kopii) —
//! ścieżka podana przez UI jest odrzucana.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::ErrorCode;
use app_core::dto::{BackupConfig, ExportResult, ImportMode, InspectResult, SessionTemplate};
use common::*;

fn export_request(sessions: Vec<String>, password: Option<&str>) -> app_core::dto::ExportRequest {
    serde_json::from_value(serde_json::json!({
        "scope": { "config_common": true, "personas": false, "casts": false, "sessions": sessions,
                   "artifacts": false, "logs": false, "config_machine": false },
        "password": password,
    }))
    .unwrap()
}

fn import_request(handle: &str, password: Option<&str>) -> app_core::dto::ImportRequest {
    serde_json::from_value(serde_json::json!({
        "handle": handle, "mode": "add", "resolutions": {}, "password": password,
    }))
    .unwrap()
}

fn handle_of(result: &InspectResult) -> String {
    match result {
        InspectResult::Inspected { handle, .. } | InspectResult::NeedsPassword { handle } => {
            handle.clone()
        }
        InspectResult::Cancelled => panic!("anulowano"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn export_import_roundtrip_through_commands() {
    let mut h = harness().await;
    let sid = h
        .core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    let turn = h
        .core
        .turns_send(sid.clone(), send("Przenieś mnie", None))
        .await
        .unwrap();
    until(&mut h.rx, ends(turn.assistant_turn_id.as_ref().unwrap())).await;
    let pack = h.dir.path().join("paczka.alfa");

    // Anulowany dialog → nic nie powstaje.
    h.shell.answer_dialog(None);
    let cancelled = h
        .core
        .transfer_export(export_request(vec![sid.clone()], None))
        .await;
    assert_eq!(cancelled.unwrap(), ExportResult::Cancelled);
    h.shell.answer_dialog(Some(pack.clone()));
    let saved = h
        .core
        .transfer_export(export_request(vec![sid.clone()], None))
        .await
        .unwrap();
    let ExportResult::Saved { path, files, .. } = saved else {
        panic!("{saved:?}")
    };
    assert!(files > 0 && std::path::Path::new(&path).is_file());

    // Druga maszyna: podgląd (dry-run) → import → rollback.
    let other = harness().await;
    other.shell.answer_dialog(Some(pack.clone()));
    let inspected = other.core.transfer_inspect(None, None).await.unwrap();
    let handle = handle_of(&inspected);
    let InspectResult::Inspected {
        items, manifest, ..
    } = inspected
    else {
        panic!("{inspected:?}")
    };
    assert!(!manifest.encrypted);
    let item = items
        .iter()
        .find(|i| i.key == sid)
        .expect("sesja w podglądzie");
    assert_eq!(item.diff, app_core::dto::ItemDiff::New);
    // Ścieżka z UI zamiast uchwytu — odrzucona (UI nie wskazuje plików).
    let raw = other
        .core
        .transfer_import(import_request(&path, None))
        .await;
    assert_eq!(raw.unwrap_err().code, ErrorCode::NotFound);
    let raw = other.core.transfer_inspect(None, Some(path.clone())).await;
    assert_eq!(raw.unwrap_err().code, ErrorCode::NotFound);
    let mut request = import_request(&handle, None);
    request.mode = ImportMode::Add;
    let imported = other.core.transfer_import(request).await.unwrap();
    // Uchwyt jednorazowy.
    let again = other
        .core
        .transfer_import(import_request(&handle, None))
        .await;
    assert_eq!(again.unwrap_err().code, ErrorCode::NotFound);
    assert!(imported.imported >= 1 && !imported.snapshot_id.is_empty());
    let snap = other.core.turns_list(sid.clone()).await.unwrap();
    let texts: Vec<&str> = snap.turns.iter().map(|t| t.text.as_str()).collect();
    assert!(texts.contains(&"Przenieś mnie"), "{texts:?}");
    assert!(
        texts.iter().any(|t| t.starts_with("Echo: Przenieś mnie")),
        "{texts:?}"
    );
    other
        .core
        .transfer_rollback(imported.snapshot_id)
        .await
        .unwrap();
    assert!(
        other.core.turns_list(sid.clone()).await.is_err(),
        "rollback usuwa import"
    );

    // Paczka szyfrowana: bez hasła → prośba o hasło; z hasłem → podgląd.
    let secret = h.dir.path().join("szyfrowana.alfa");
    h.shell.answer_dialog(Some(secret.clone()));
    let saved = h
        .core
        .transfer_export(export_request(vec![], Some("tajne hasło 123")))
        .await;
    assert!(matches!(saved.unwrap(), ExportResult::Saved { .. }));
    other.shell.answer_dialog(Some(secret.clone()));
    let need = other.core.transfer_inspect(None, None).await.unwrap();
    assert!(
        matches!(need, InspectResult::NeedsPassword { .. }),
        "{need:?}"
    );
    // Złe hasło → znowu prośba o hasło z nowym uchwytem.
    let wrong = other
        .core
        .transfer_inspect(Some("inne hasło 456".into()), Some(handle_of(&need)))
        .await
        .unwrap();
    assert!(
        matches!(wrong, InspectResult::NeedsPassword { .. }),
        "{wrong:?}"
    );
    let ok = other
        .core
        .transfer_inspect(Some("tajne hasło 123".into()), Some(handle_of(&wrong)))
        .await;
    assert!(matches!(ok.unwrap(), InspectResult::Inspected { manifest, .. } if manifest.encrypted));

    // Regresja CX-a (AGENTS.md): sekrety nigdy w `.alfa` — komendy eksportu sekretów nie ma.
    assert!(
        !app_core::COMMANDS.contains(&"transfer_export_secrets"),
        "eksport sekretów do paczki"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restore_from_backup_list_issues_a_short_lived_handle() {
    let h = harness().await;
    let core = &h.core;
    let start = core.backups_status().await.unwrap();
    let dir = h.dir.path().join("Kopie");
    std::fs::create_dir_all(&dir).unwrap();
    h.shell.answer_dialog(Some(dir.clone()));
    core.backups_choose_dir().await.unwrap();
    core.backups_configure(BackupConfig {
        enabled: true,
        ..start.config
    })
    .await
    .unwrap();
    let view = core.backups_run_now().await.unwrap();
    let file = view.entries[0].file.clone();

    // Tylko nazwa z listy kopii; ścieżka ani plik spoza katalogu kopii nie dają uchwytu.
    let outside = core.backups_restore(view.entries[0].path.clone()).await;
    assert_eq!(outside.unwrap_err().code, ErrorCode::NotFound);
    let missing = core.backups_restore("..\\inna.alfa".into()).await;
    assert_eq!(missing.unwrap_err().code, ErrorCode::NotFound);

    let handle = core.backups_restore(file).await.unwrap();
    assert!(
        !handle.contains("Kopie"),
        "uchwyt nie zdradza ścieżki: {handle}"
    );
    let inspected = core.transfer_inspect(None, Some(handle.clone())).await;
    assert!(
        matches!(inspected, Ok(InspectResult::Inspected { .. })),
        "{inspected:?}"
    );
    let reused = core.transfer_inspect(None, Some(handle)).await;
    assert_eq!(reused.unwrap_err().code, ErrorCode::NotFound);
}
