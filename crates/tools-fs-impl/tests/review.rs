//! Regresje z przeglądu narzędzi plikowych (2026-10-08, docs/STATUS.md: T-01, T-05).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::harness;
use serde_json::json;
use tools_fs_contract::FsToolKind as K;

/// T-01: nazwy urządzeń Windows nie są plikami — odczyt mógł zawisnąć na konsoli, a zapis do `nul`
/// zgłaszał sukces, choć nic nie powstawało.
#[tokio::test]
async fn windows_device_names_are_refused_before_any_file_access() {
    let h = harness(&[("/Users/ala/Documents/notatka.txt", "treść")]);
    for path in [
        "con",
        "NUL",
        "com1",
        "aux.log",
        "podkatalog/nul.txt",
        "LPT1",
    ] {
        let read = h.call(K::Read, json!({ "path": path })).await;
        assert!(
            !read.is_ok(),
            "odczyt „{path}” powinien być odrzucony: {read:?}"
        );
        let write = h
            .call(
                K::Write,
                json!({ "path": path, "content": "x", "mode": "overwrite" }),
            )
            .await;
        assert!(
            !write.is_ok(),
            "zapis „{path}” powinien być odrzucony: {write:?}"
        );
    }
    assert!(h.file("/Users/ala/Documents/nul").is_none());
    assert!(h.file("/Users/ala/Documents/NUL").is_none());
    // Zwykłe nazwy zawierające te litery działają dalej.
    let ok = h
        .call(
            K::Write,
            json!({ "path": "console.txt", "content": "ok", "mode": "create" }),
        )
        .await;
    assert!(ok.is_ok(), "{ok:?}");
}

/// T-05: plik, którego treści nie dało się przeszukać (binarny), nie może wyglądać jak „brak trafień”.
#[tokio::test]
async fn search_reports_files_whose_content_was_not_searched() {
    let h = harness(&[
        ("/Users/ala/Documents/dane.txt", "szukana fraza tutaj"),
        ("/Users/ala/Documents/obraz.txt", "\u{0}\u{0}\u{1}\u{2}"),
    ]);
    let out = h
        .call(
            K::Search,
            json!({ "root": ".", "pattern": "*.txt", "content": "fraza" }),
        )
        .await;
    assert!(out.is_ok(), "{out:?}");
    assert!(out.text.contains("dane.txt"), "{}", out.text);
    assert!(
        out.text.contains("Nie przeszukano treści 1 plików"),
        "model ma wiedzieć o pominiętym pliku: {}",
        out.text
    );
    // Bez pominiętych plików notatki nie ma.
    let clean = harness(&[("/Users/ala/Documents/dane.txt", "szukana fraza tutaj")]);
    let out = clean
        .call(
            K::Search,
            json!({ "root": ".", "pattern": "*.txt", "content": "fraza" }),
        )
        .await;
    assert!(!out.text.contains("Nie przeszukano"), "{}", out.text);
}
