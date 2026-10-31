//! Dług katalogu funkcji (docs/plan/45-testy-ai.md): eksport i import paczki umiejętności na
//! rdzeniu. Okna „Zapisz jako” / „Otwórz” obsługuje powłoka bez okien (`HeadlessShell`) —
//! ścieżki podaje test, wszystko w katalogu tymczasowym. Import z zewnątrz zawsze kończy się
//! kwarantanną (propozycja), nigdy instalacją.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::ErrorCode;
use app_core::dto::{ExportResult, SkillStateView};
use common::{Harness, harness};
use serde_json::json;

fn manifest() -> serde_json::Value {
    json!({
        "id": "porzadki-pobranych",
        "version": "1.0.0",
        "name": "Porządki w Pobranych",
        "description": "Sortuje folder Pobrane według typu, bez usuwania plików.",
        "keywords": ["pobrane", "sortowanie"],
        "required_tools": ["fs_list", "fs_move"],
        "required_capabilities": ["fs.read", "fs.write"],
        "parameters": {
            "type": "object",
            "properties": {"folder": {"type": "string", "maxLength": 200}},
            "required": ["folder"],
            "additionalProperties": false
        },
        "prompt": "Uporządkuj folder {{folder}} według typu.",
        "acceptance": [
            {"name": "folder w celu", "params": {"folder": "C:/Users/ala/Downloads"},
             "expect_in_goal": ["C:/Users/ala/Downloads"]},
            {"name": "brak folderu", "params": {}, "expect_rejected": true}
        ]
    })
}

/// Umiejętność zaproponowana, przejrzana i zatwierdzona hashem (zainstalowana).
async fn install(h: &Harness) -> String {
    let proposed = h.core.skills_propose(manifest()).await.unwrap();
    let review = h
        .core
        .skills_review(proposed.id.clone(), proposed.version.clone())
        .await
        .unwrap();
    let installed = h
        .core
        .skills_approve(proposed.id.clone(), proposed.version, review.skill.hash)
        .await
        .unwrap();
    assert_eq!(installed.state, SkillStateView::Installed);
    installed.id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skills_export_writes_bundle_where_owner_chose() {
    let h = harness().await;
    let id = install(&h).await;
    // Propozycja bez zatwierdzenia nie trafia do paczki.
    let mut other = manifest();
    other["id"] = json!("tylko-propozycja");
    h.core.skills_propose(other).await.unwrap();

    let out = h.dir.path().join("eksport").join("paczka.skills.json");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    h.shell.answer_dialog(Some(out.clone()));
    let result = h.core.skills_export().await.unwrap();
    let ExportResult::Saved { path, files, bytes } = result else {
        panic!("oczekiwano zapisu: {result:?}");
    };
    assert_eq!(path, out.to_string_lossy());
    assert_eq!(files, 1, "tylko zainstalowane");
    let written = std::fs::read(&out).unwrap();
    assert_eq!(bytes, written.len() as u64);
    let text = String::from_utf8_lossy(&written);
    assert!(
        text.contains(&id) && !text.contains("tylko-propozycja"),
        "{text}"
    );
    assert!(
        h.shell.calls().iter().any(|c| c.starts_with("pick_save:")),
        "{:?}",
        h.shell.calls()
    );

    // Anulowane okno — nic nie zapisane; bez okna (tryb bezgłowy) — „niedostępne”.
    std::fs::remove_file(&out).unwrap();
    h.shell.answer_dialog(None);
    assert_eq!(
        h.core.skills_export().await.unwrap(),
        ExportResult::Cancelled
    );
    assert!(!out.exists());
    let err = h.core.skills_export().await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Unavailable, "{err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skills_import_puts_exported_bundle_into_quarantine() {
    let source = harness().await;
    let id = install(&source).await;
    let bundle = source.dir.path().join("paczka.skills.json");
    source.shell.answer_dialog(Some(bundle.clone()));
    source.core.skills_export().await.unwrap();

    let h = harness().await;
    h.shell.answer_dialog(Some(bundle));
    let imported = h.core.skills_import().await.unwrap();
    assert!(imported.skipped.is_empty(), "{imported:?}");
    assert_eq!(imported.proposed.len(), 1);
    let skill = &imported.proposed[0];
    assert_eq!(skill.id, id);
    assert_eq!(skill.state, SkillStateView::Quarantined, "kwarantanna");
    let listed = h.core.skills_list().await.unwrap();
    let on_list = listed.iter().find(|s| s.id == id).unwrap();
    assert_eq!(
        on_list.state,
        SkillStateView::Quarantined,
        "nie zainstalowana"
    );
    assert!(h.shell.calls().iter().any(|c| c == "pick_open"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skills_import_converts_skill_md_and_refuses_garbage() {
    let h = harness().await;
    let dir = h.dir.path().join("Pobrane");
    std::fs::create_dir_all(&dir).unwrap();
    let skill_md = dir.join("SKILL.md");
    std::fs::write(
        &skill_md,
        "---\nname: Raport tygodniowy\ndescription: Zbiera notatki z tygodnia w krótki raport.\n---\n\n# Kroki\n\nPrzeczytaj notatki i napisz raport.\n",
    )
    .unwrap();
    h.shell.answer_dialog(Some(skill_md));
    let imported = h.core.skills_import().await.unwrap();
    assert_eq!(imported.proposed.len(), 1, "{imported:?}");
    let skill = &imported.proposed[0];
    assert_eq!(skill.name, "Raport tygodniowy");
    assert_eq!(skill.state, SkillStateView::Quarantined);
    assert!(!skill.trusted, "z zewnątrz — niezaufana");

    // Anulowane okno — pusty wynik bez zmian na liście.
    let count = h.core.skills_list().await.unwrap().len();
    h.shell.answer_dialog(None);
    let cancelled = h.core.skills_import().await.unwrap();
    assert!(cancelled.proposed.is_empty() && cancelled.skipped.is_empty());

    let garbage = dir.join("smieci.skills.json");
    std::fs::write(&garbage, b"{\"to\": \"nie paczka\"}").unwrap();
    let broken_md = dir.join("zly.md");
    std::fs::write(&broken_md, "---\nname: Bez końca nagłówka\n").unwrap();
    for path in [garbage, broken_md] {
        h.shell.answer_dialog(Some(path.clone()));
        let err = h.core.skills_import().await.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{path:?}: {err:?}");
    }
    let missing = dir.join("nie-ma.skills.json");
    h.shell.answer_dialog(Some(missing));
    let err = h.core.skills_import().await.unwrap_err();
    assert_eq!(err.code, ErrorCode::Storage, "{err:?}");
    assert_eq!(h.core.skills_list().await.unwrap().len(), count);
    let headless = h.core.skills_import().await.unwrap_err();
    assert_eq!(headless.code, ErrorCode::Unavailable);
}
