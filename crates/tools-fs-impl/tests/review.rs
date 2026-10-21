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
        // Sama odmowa to za mało — atrapa nie ma takiego pliku, więc odczyt i tak by się nie udał.
        assert!(
            format!("{read:?}").contains("niedozwolon"),
            "odczyt „{path}” ma być odrzucony jako niedozwolona ścieżka: {read:?}"
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

/// T-02 i T-03: dopisywanie liczy limit dla pliku po zapisie (bez wczytywania ponad limit),
/// a `overwrite` i `create` nie potrzebują starej treści — tylko wiedzy, czy plik istnieje.
#[tokio::test]
async fn append_respects_limit_and_other_modes_ignore_old_content() {
    const DOC: &str = "/Users/ala/Documents/dziennik.txt";
    let big = "a".repeat(6 * 1024 * 1024);
    let h = harness(&[(DOC, big.as_str())]);
    let over = h
        .call(
            K::Write,
            json!({ "path": DOC, "content": big, "mode": "append" }),
        )
        .await;
    assert!(!over.is_ok(), "dopisanie ponad limit: {:?}", over.status);
    assert!(over.text.contains("limit zapisu"), "{}", over.text);
    assert_eq!(h.file(DOC).map(|f| f.len()), Some(big.len()));

    let small = h
        .call(
            K::Write,
            json!({ "path": DOC, "content": "-koniec", "mode": "append" }),
        )
        .await;
    assert!(small.is_ok(), "{:?}", small.status);
    let after = h.file(DOC).unwrap();
    assert_eq!(after.len(), big.len() + 7);
    assert!(after.ends_with("a-koniec"));

    let exists = h
        .call(
            K::Write,
            json!({ "path": DOC, "content": "x", "mode": "create" }),
        )
        .await;
    assert!(!exists.is_ok(), "create na istniejącym pliku ma odmówić");
    assert_eq!(h.file(DOC).map(|f| f.len()), Some(big.len() + 7));

    let replaced = h
        .call(
            K::Write,
            json!({ "path": DOC, "content": "nowa", "mode": "overwrite" }),
        )
        .await;
    assert!(replaced.is_ok(), "{:?}", replaced.status);
    assert_eq!(h.file(DOC).as_deref(), Some("nowa"));
}

/// Przegląd 2026-10-21: `fs_read` redaguje zwykły kod (`secret: SecretName`), a model bez
/// narzędzia edycji przepisywał cały plik i wstawiał `[ZREDAGOWANO]` do źródeł.
#[tokio::test]
async fn write_refuses_redaction_markers_taken_from_a_read() {
    const SRC: &str = "/Users/ala/Documents/konto.rs";
    let code = "pub struct K {\n    pub secret: SecretName,\n}\n";
    let h = harness(&[(SRC, code)]);
    let read = h.call(K::Read, json!({ "path": SRC })).await;
    let shown = read.data["content"].as_str().unwrap().to_owned();
    assert!(shown.contains("[ZREDAGOWANO]"), "{shown}");
    for (path, mode) in [(SRC, "overwrite"), (SRC, "append"), ("kopia.rs", "create")] {
        let out = h
            .call(
                K::Write,
                json!({ "path": path, "content": shown.replace("K {", "K2 {"), "mode": mode }),
            )
            .await;
        assert!(!out.is_ok(), "{mode}: {:?}", out.status);
        assert!(out.text.contains("znacznik [ZREDAGOWANO]"), "{}", out.text);
    }
    assert_eq!(h.file(SRC).as_deref(), Some(code));
    assert!(h.file("/Users/ala/Documents/kopia.rs").is_none());
    // Plik, który już zawiera znacznik (np. dokumentacja), można przepisać bez nowych znaczników.
    const DOC: &str = "/Users/ala/Documents/opis.md";
    let h = harness(&[(DOC, "Wynik: [ZREDAGOWANO]\n")]);
    let same = h
        .call(
            K::Write,
            json!({ "path": DOC, "content": "Opis: [ZREDAGOWANO]\n", "mode": "overwrite" }),
        )
        .await;
    assert!(same.is_ok(), "{:?}", same.status);
    let plain = h
        .call(
            K::Write,
            json!({ "path": DOC, "content": "bez znacznika", "mode": "append" }),
        )
        .await;
    assert!(plain.is_ok(), "{:?}", plain.status);
}
