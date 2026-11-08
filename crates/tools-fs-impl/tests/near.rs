//! Podpowiedź przy literówce w ścieżce (próba obowiązku „tidy”, 1.11): tylko w katalogu
//! roboczym, nigdy na deny-liście, bez otwierania czegokolwiek.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::harness;
use serde_json::json;
use tools_common_contract::{ToolErrorKind, ToolStatus};
use tools_fs_contract::FsToolKind as K;

fn files() -> common::Harness {
    harness(&[
        ("/Users/ala/Documents/Raporty/marzec.txt", "m"),
        ("/Users/ala/Documents/.ssh/id_rsa", "klucz"),
        ("/Users/ala/Pulpit/notatka.txt", "n"),
    ])
}

#[tokio::test]
async fn typo_in_workdir_name_gets_a_hint() {
    let h = files();
    let st = h
        .call(
            K::Stat,
            json!({"path": "/Users/ala/Dokuments/Raporty/marzec.txt"}),
        )
        .await;
    assert_eq!(st.data["exists"], false);
    assert!(
        st.text.contains("Czy chodziło o") && st.text.contains("marzec.txt"),
        "{}",
        st.text
    );
}

#[tokio::test]
async fn typo_above_the_workdir_gets_a_hint() {
    // „Users” → „Usres” powyżej katalogu roboczego (pomiar F3: „…\Local\Tepm\…”).
    let h = files();
    let st = h
        .call(
            K::Stat,
            json!({"path": "/Usres/ala/Documents/Raporty/marzec.txt"}),
        )
        .await;
    assert!(st.text.contains("Czy chodziło o"), "{}", st.text);
    let too_many = h
        .call(
            K::Stat,
            json!({"path": "/Usres/aal/Dokuments/Raporty/marzec.txt"}),
        )
        .await;
    assert!(
        !too_many.text.contains("Czy chodziło o"),
        "{}",
        too_many.text
    );
}

#[tokio::test]
async fn typo_inside_workdir_gets_a_hint_on_read_and_list() {
    let h = files();
    // Odczyt (bez skutków) od razu z poprawioną ścieżką i informacją, co odczytano (pomiar F3 fs-09).
    let read = h.call(K::Read, json!({"path": "Raprty/marzec.txt"})).await;
    assert!(read.is_ok(), "{read:?}");
    assert!(
        read.text.contains("literówka w ścieżce") && read.text.contains("Raporty"),
        "{}",
        read.text
    );
    assert!(read.data["path"].as_str().unwrap().contains("Raporty"));
    // Dwie literówki ponad limit poprawek — zwykły błąd.
    let far = h.call(K::Read, json!({"path": "Rxxxxxx/mxxxxc.txt"})).await;
    assert_eq!(
        far.status,
        ToolStatus::Failed {
            error: ToolErrorKind::NotFound
        }
    );
    let list = h.call(K::List, json!({"path": "Raport"})).await;
    assert!(list.text.contains("Czy chodziło o"), "{}", list.text);
}

#[tokio::test]
async fn no_hint_outside_workdir_or_on_the_deny_list() {
    let h = files();
    let outside = h
        .call(K::Stat, json!({"path": "/Users/ala/Pulpt/notatka.txt"}))
        .await;
    assert!(!outside.text.contains("Czy chodziło o"), "{}", outside.text);
    let secret = h.call(K::Stat, json!({"path": ".sh/id_rsa"})).await;
    assert!(!secret.text.contains("Czy chodziło o"), "{}", secret.text);
    let far = h
        .call(K::Stat, json!({"path": "Zupełnie inne/marzec.txt"}))
        .await;
    assert!(!far.text.contains("Czy chodziło o"), "{}", far.text);
}

#[tokio::test]
async fn copy_or_move_to_the_trash_path_points_to_fs_delete() {
    let h = files();
    let copy = h
        .call(
            K::Copy,
            json!({"from": "Raporty/marzec.txt", "to": "/Kosz/marzec.txt"}),
        )
        .await;
    assert_eq!(
        copy.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
    assert!(copy.text.contains("fs_delete"), "{}", copy.text);
    let mv = h
        .call(
            K::Move,
            json!({"from": "Raporty/marzec.txt", "to": "C:/$Recycle.Bin/m.txt"}),
        )
        .await;
    assert!(mv.text.contains("fs_delete"), "{}", mv.text);
    // Zwykły katalog „Kosz” w katalogu roboczym zostaje katalogiem.
    let plain = h
        .call(
            K::Copy,
            json!({"from": "Raporty/marzec.txt", "to": "Kosz/marzec.txt"}),
        )
        .await;
    assert!(!plain.text.contains("Kosz nie jest"), "{}", plain.text);
}
