//! Dług katalogu funkcji (docs/plan/45-testy-ai.md): rejestr projektów na rdzeniu — lista łączy
//! projekty zapisane z projektami przypisanymi do rozmów (z licznikiem), zmiana nazwy przenosi
//! katalog roboczy i rozmowy, usunięcie odpina rozmowy, a zła nazwa jest odrzucana bez zmian.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use app_core::ErrorCode;
use app_core::dto::{ProjectInfo, SessionTemplate};
use common::{Harness, harness};

async fn session_in(h: &Harness, project: Option<&str>) -> String {
    let id = h
        .core
        .sessions_create(SessionTemplate::Empty)
        .await
        .unwrap()
        .id;
    h.core
        .sessions_set_project(id.clone(), project.map(str::to_owned))
        .await
        .unwrap();
    id
}

async fn project_of(h: &Harness, session: &str) -> Option<String> {
    let list = h.core.sessions_list().await.unwrap();
    let summary = list.into_iter().find(|s| s.id == session).unwrap();
    summary.project.map(|p| p.name)
}

fn find<'a>(list: &'a [ProjectInfo], name: &str) -> Option<&'a ProjectInfo> {
    list.iter().find(|p| p.name == name)
}

fn workdir(h: &Harness, name: &str) -> String {
    let dir = h.dir.path().join("projekty").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir.to_string_lossy().into_owned()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn projects_list_merges_saved_projects_with_session_projects() {
    let h = harness().await;
    assert!(
        h.core.projects_list().await.unwrap().is_empty(),
        "świeży profil nie ma projektów"
    );
    let dir = workdir(&h, "alfa");
    h.core
        .projects_save("Alfa".into(), Some(dir))
        .await
        .unwrap();
    session_in(&h, Some("beta")).await;
    session_in(&h, Some("beta")).await;
    session_in(&h, None).await;

    let list = h.core.projects_list().await.unwrap();
    let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Alfa", "beta"], "kolejność bez wielkości liter");
    let alfa = find(&list, "Alfa").unwrap();
    assert_eq!(alfa.session_count, 0, "zapisany bez rozmów");
    assert!(
        alfa.workdir.as_deref().unwrap().ends_with("alfa"),
        "{alfa:?}"
    );
    let beta = find(&list, "beta").unwrap();
    assert_eq!(
        (beta.session_count, beta.workdir.as_deref()),
        (2, None),
        "projekt tylko z rozmów"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn projects_rename_moves_workdir_and_sessions() {
    let h = harness().await;
    let dir = workdir(&h, "stary");
    h.core
        .projects_save("Stary".into(), Some(dir.clone()))
        .await
        .unwrap();
    let a = session_in(&h, Some("Stary")).await;
    let other = session_in(&h, Some("Inny")).await;

    let list = h
        .core
        .projects_rename("  Stary ".into(), "Nowy".into())
        .await
        .unwrap();
    assert!(
        find(&list, "Stary").is_none(),
        "stara nazwa znika: {list:?}"
    );
    let renamed = find(&list, "Nowy").unwrap();
    assert_eq!(renamed.session_count, 1);
    assert_eq!(renamed.workdir.as_deref(), Some(dir.as_str()));
    assert_eq!(project_of(&h, &a).await.as_deref(), Some("Nowy"));
    assert_eq!(project_of(&h, &other).await.as_deref(), Some("Inny"));
    assert_eq!(h.core.projects_list().await.unwrap(), list, "zapisane");

    // Zmiana na nazwę istniejącego projektu łączy rozmowy w jednym projekcie.
    let merged = h
        .core
        .projects_rename("Inny".into(), "Nowy".into())
        .await
        .unwrap();
    assert!(find(&merged, "Inny").is_none());
    assert_eq!(find(&merged, "Nowy").unwrap().session_count, 2);
    assert_eq!(project_of(&h, &other).await.as_deref(), Some("Nowy"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn projects_rename_refuses_bad_names_without_changes() {
    let h = harness().await;
    let a = session_in(&h, Some("Raport")).await;
    let before = h.core.projects_list().await.unwrap();
    for (old, new) in [("Raport", "   "), ("", "Nowa"), ("Raport", &"x".repeat(81))] {
        let err = h
            .core
            .projects_rename(old.into(), new.into())
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{old:?} → {new:?}");
    }
    assert_eq!(h.core.projects_list().await.unwrap(), before);
    assert_eq!(project_of(&h, &a).await.as_deref(), Some("Raport"));
    // Ta sama nazwa (po przycięciu) to brak zmiany, nie błąd.
    let same = h
        .core
        .projects_rename("Raport".into(), " Raport ".into())
        .await
        .unwrap();
    assert_eq!(same, before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn projects_remove_forgets_project_and_detaches_sessions() {
    let h = harness().await;
    let dir = workdir(&h, "do-usuniecia");
    h.core
        .projects_save("Do usunięcia".into(), Some(dir))
        .await
        .unwrap();
    h.core.projects_save("Zostaje".into(), None).await.unwrap();
    let a = session_in(&h, Some("Do usunięcia")).await;
    let b = session_in(&h, Some("Zostaje")).await;

    let list = h
        .core
        .projects_remove(" Do usunięcia ".into())
        .await
        .unwrap();
    let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Zostaje"]);
    assert_eq!(project_of(&h, &a).await, None, "rozmowa odpięta");
    assert_eq!(project_of(&h, &b).await.as_deref(), Some("Zostaje"));
    assert_eq!(h.core.projects_list().await.unwrap(), list);

    // Usunięcie nieznanego projektu niczego nie psuje; pusta nazwa — odmowa.
    let again = h.core.projects_remove("Nie ma".into()).await.unwrap();
    assert_eq!(again, list);
    let err = h.core.projects_remove("  ".into()).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(h.core.projects_list().await.unwrap(), list);
}
