//! Nadpisanie pliku, którego przebieg nie odczytał (pomiar F3 fs-11, fs-13, 2.11): lekki model
//! zastępował treść umowy jednym zdaniem bez odczytu. Odczyt albo własny zapis w tym samym
//! przebiegu odblokowuje nadpisanie; nowy plik, pusty plik i wywołanie spoza przebiegu — bez zmian.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{ctx, harness};
use core_bus_contract::RunId;
use serde_json::json;
use tools_common_contract::{ToolCtx, ToolErrorKind, ToolStatus};
use tools_fs_contract::FsToolKind as K;

const DOC: &str = "umowa.txt";
const PATH: &str = "/Users/ala/Documents/umowa.txt";

fn in_run(id: &str) -> ToolCtx {
    let mut c = ctx();
    c.run = Some(RunId::new(id));
    c
}

fn overwrite(content: &str) -> serde_json::Value {
    json!({"path": DOC, "content": content, "mode": "overwrite"})
}

#[tokio::test]
async fn overwrite_without_read_is_refused_and_read_unlocks_it() {
    let h = harness(&[(PATH, "Strona A: Kowalski\n")]);
    let run = in_run("r1");
    let blind = h
        .call_ctx(K::Write, overwrite("Zawartość umowy"), &run)
        .await;
    assert_eq!(
        blind.status,
        ToolStatus::Failed {
            error: ToolErrorKind::InvalidArgs
        }
    );
    assert!(blind.text.contains("fs_read"), "{}", blind.text);
    assert_eq!(h.file(PATH).as_deref(), Some("Strona A: Kowalski\n"));

    let read = h.call_ctx(K::Read, json!({"path": DOC}), &run).await;
    assert!(read.is_ok(), "{read:?}");
    let ok = h
        .call_ctx(K::Write, overwrite("Strona A: Wiśniewski\n"), &run)
        .await;
    assert!(ok.is_ok(), "{ok:?}");
    assert_eq!(h.file(PATH).as_deref(), Some("Strona A: Wiśniewski\n"));

    // Inny przebieg tego pliku nie odczytał.
    let other = h.call_ctx(K::Write, overwrite("x"), &in_run("r2")).await;
    assert!(!other.is_ok());
}

#[tokio::test]
async fn own_write_new_file_empty_file_and_no_run_are_not_blocked() {
    let h = harness(&[("/Users/ala/Documents/pusty.txt", "")]);
    let run = in_run("r1");
    let new = h.call_ctx(K::Write, overwrite("1"), &run).await;
    assert!(new.is_ok(), "nowy plik: {new:?}");
    let again = h.call_ctx(K::Write, overwrite("2"), &run).await;
    assert!(again.is_ok(), "własny zapis: {again:?}");
    let empty = h
        .call_ctx(
            K::Write,
            json!({"path": "pusty.txt", "content": "a", "mode": "overwrite"}),
            &run,
        )
        .await;
    assert!(empty.is_ok(), "pusty plik: {empty:?}");
    let ui = h.call(K::Write, overwrite("3")).await;
    assert!(ui.is_ok(), "poza przebiegiem: {ui:?}");
}

#[tokio::test]
async fn append_needs_no_read() {
    let h = harness(&[(PATH, "a\n")]);
    let out = h
        .call_ctx(
            K::Write,
            json!({"path": DOC, "content": "b\n", "mode": "append"}),
            &in_run("r1"),
        )
        .await;
    assert!(out.is_ok(), "{out:?}");
    assert_eq!(h.file(PATH).as_deref(), Some("a\nb\n"));
}

/// Przegląd 2.11: dopisanie i odczyt częściowy nie pokazują całej treści — nie odblokowują nadpisania.
#[tokio::test]
async fn append_or_partial_read_does_not_unlock_overwrite() {
    let h = harness(&[(PATH, "Strona A: Kowalski\nStrona B: Nowak\n")]);
    let run = in_run("r1");
    let add = h
        .call_ctx(
            K::Write,
            json!({"path": DOC, "content": "Podpis\n", "mode": "append"}),
            &run,
        )
        .await;
    assert!(add.is_ok(), "{add:?}");
    let blind = h.call_ctx(K::Write, overwrite("x"), &run).await;
    assert!(!blind.is_ok(), "append nie pokazał treści: {blind:?}");
    let part = h
        .call_ctx(K::Read, json!({"path": DOC, "max_bytes": 5}), &run)
        .await;
    assert!(part.is_ok(), "{part:?}");
    let still = h.call_ctx(K::Write, overwrite("x"), &run).await;
    assert!(!still.is_ok(), "odczyt częściowy: {still:?}");
    let full = h.call_ctx(K::Read, json!({"path": DOC}), &run).await;
    assert!(full.is_ok(), "{full:?}");
    let ok = h.call_ctx(K::Write, overwrite("x"), &run).await;
    assert!(ok.is_ok(), "po pełnym odczycie: {ok:?}");
}
