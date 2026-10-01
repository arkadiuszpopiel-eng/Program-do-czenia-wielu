//! Eksport → import `.alfa` przez komendy (moduł `transfer`): dialogi powłoki, podgląd
//! (dry-run), tryb, snapshot i rollback, paczka szyfrowana hasłem, sekrety tylko jawnie z hasłem.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::ErrorCode;
use app_core::dto::{ExportResult, ImportMode, InspectResult, SessionTemplate};
use common::*;

fn export_request(sessions: Vec<String>, password: Option<&str>) -> app_core::dto::ExportRequest {
    serde_json::from_value(serde_json::json!({
        "scope": { "config_common": true, "personas": false, "casts": false, "sessions": sessions,
                   "artifacts": false, "logs": false, "config_machine": false },
        "password": password,
    }))
    .unwrap()
}

fn import_request(path: &str, password: Option<&str>) -> app_core::dto::ImportRequest {
    serde_json::from_value(serde_json::json!({
        "path": path, "mode": "add", "resolutions": {}, "password": password,
    }))
    .unwrap()
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
    let mut request = import_request(&path, None);
    request.mode = ImportMode::Add;
    let imported = other.core.transfer_import(request).await.unwrap();
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
    let spath = secret.to_string_lossy().into_owned();
    let need = other
        .core
        .transfer_inspect(None, Some(spath.clone()))
        .await
        .unwrap();
    assert!(
        matches!(need, InspectResult::NeedsPassword { .. }),
        "{need:?}"
    );
    let ok = other
        .core
        .transfer_inspect(Some("tajne hasło 123".into()), Some(spath))
        .await;
    assert!(matches!(ok.unwrap(), InspectResult::Inspected { manifest, .. } if manifest.encrypted));

    // Sekrety: tylko jawnie i z hasłem ≥ 8 znaków.
    let weak = h
        .core
        .transfer_export_secrets("krótkie".into())
        .await
        .unwrap_err();
    assert_eq!(weak.code, ErrorCode::InvalidInput, "{weak:?}");
    h.shell
        .answer_dialog(Some(h.dir.path().join("sekrety.alfa")));
    let secrets = h
        .core
        .transfer_export_secrets("długie hasło sekretów".into())
        .await;
    assert!(matches!(secrets.unwrap(), ExportResult::Saved { .. }));
}
