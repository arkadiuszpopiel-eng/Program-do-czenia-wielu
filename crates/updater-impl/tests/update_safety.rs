//! Odrzucanie złych aktualizacji (100% bez śladów w `versions\` i `current.json`): zły podpis,
//! zła suma, obcy klucz, podpis innej wersji (podmiana etykiety = próba downgrade'u), wersja
//! ≤ bieżącej bez jawnego rollbacku użytkownika, path traversal, zip-bomb, dowiązania,
//! duplikaty nazw, brak/niezgodny `version.json`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::{Cursor, Write};

use common::flow::{Flow, noise, package, v};
use updater_contract::{InstallIntent, Release, UpdatePhase, Updater, UpdaterError};
use zip::write::SimpleFileOptions;

/// Po odrzuceniu: aktywna bez zmian, brak katalogu wersji, brak plików częściowych i roboczych.
fn assert_untouched(f: &Flow, version: &str) {
    assert_eq!(f.active(), v("1.0.0"));
    assert!(!f.updater.layout().version_dir(&v(version)).exists());
    assert!(f.staging_files().is_empty(), "{:?}", f.staging_files());
    let leftovers: Vec<String> = std::fs::read_dir(&f.updater.layout().versions)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    assert_eq!(f.service.status().phase, UpdatePhase::Failed);
}

async fn try_install(f: &Flow, release: Release, bytes: Vec<u8>) -> UpdaterError {
    f.publish("stable", &[(release, bytes)]);
    f.service.check().await.unwrap();
    f.service
        .download(InstallIntent::Update)
        .await
        .expect_err("paczka powinna zostać odrzucona")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_signature_hash_key_and_version_tag_are_rejected() {
    let bytes = package("1.1.0", &[]);
    type Make = fn(&Flow, &[u8]) -> Release;
    let cases: [(&str, Make); 4] = [
        ("zły podpis", |f, b| {
            let mut r = f.signed("1.1.0", "1.1.0", b);
            r.minisign = f.signed("1.1.0", "1.1.0", b"inna tresc").minisign;
            r
        }),
        ("zła suma", |f, b| {
            let mut r = f.signed("1.1.0", "1.1.0", b);
            r.sha256 = common::sha(b"inna tresc");
            r
        }),
        ("obcy klucz", |f, b| {
            let mut r = f.signed("1.1.0", "1.1.0", b);
            let trusted = "timestamp:1\tversion:1.1.0";
            r.minisign = common::sign(&f.h.other, b, trusted);
            r
        }),
        // Stara paczka (podpis wiąże 0.9.0) podana w manifeście jako 1.1.0.
        ("podpis innej wersji", |f, b| f.signed("1.1.0", "0.9.0", b)),
    ];
    for (name, make) in cases {
        let f = Flow::new("1.0.0").await;
        let release = make(&f, &bytes);
        let err = try_install(&f, release, bytes.clone()).await;
        match name {
            "zła suma" => assert_eq!(err, UpdaterError::HashMismatch, "{name}"),
            _ => assert!(
                matches!(err, UpdaterError::SignatureInvalid { .. }),
                "{name}: {err:?}"
            ),
        }
        assert_untouched(&f, "1.1.0");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn downgrade_needs_explicit_user_rollback() {
    let f = Flow::new("1.0.0").await;
    let old = package("0.9.0", &[]);
    let same = package("1.0.0", &[]);
    let r_old = f.signed("0.9.0", "0.9.0", &old);
    let r_same = f.signed("1.0.0", "1.0.0", &same);
    f.publish("stable", &[(r_old, old), (r_same, same)]);
    let s = f.service.check().await.unwrap();
    assert_eq!(
        s.phase,
        UpdatePhase::UpToDate,
        "starsza ani równa nie są proponowane"
    );
    f.service.offer(&v("0.9.0")).await.unwrap();
    let err = f.service.download(InstallIntent::Update).await.unwrap_err();
    assert!(matches!(err, UpdaterError::Downgrade { .. }), "{err:?}");
    assert_untouched(&f, "0.9.0");
    // Jawny powrót użytkownika do starszego wydania (podpis nadal wiąże wersję).
    f.service.offer(&v("0.9.0")).await.unwrap();
    let s = f
        .service
        .download(InstallIntent::UserRollback)
        .await
        .unwrap();
    assert_eq!((s.phase, s.ready), (UpdatePhase::Ready, Some(v("0.9.0"))));
    assert_eq!(f.updater.select_launch().unwrap().version, v("0.9.0"));
}

fn raw_zip(entries: &[(&str, Vec<u8>)], symlink: Option<&str>) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in entries {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(data).unwrap();
    }
    if let Some(link) = symlink {
        zip.add_symlink(link, "/etc/passwd", opts).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn base(version: &str) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("alfa-desktop.exe", b"MZ".to_vec()),
        (
            "version.json",
            format!("{{\"version\":\"{version}\"}}").into_bytes(),
        ),
    ]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsafe_archives_are_rejected() {
    let with = |extra: &[(&'static str, Vec<u8>)]| {
        let mut e = base("1.1.0");
        e.extend(extra.iter().cloned());
        raw_zip(&e, None)
    };
    let bomb = vec![0u8; 8 << 20];
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("path traversal", with(&[("../poza.txt", b"x".to_vec())])),
        (
            "głęboki traversal",
            with(&[("a/../../poza.txt", b"x".to_vec())]),
        ),
        ("ścieżka bezwzględna", with(&[("/etc/x", b"x".to_vec())])),
        ("dysk Windows", with(&[("C:/Windows/x.dll", b"x".to_vec())])),
        ("strumień NTFS", with(&[("alfa.exe:zone", b"x".to_vec())])),
        (
            "nazwa urządzenia",
            with(&[("zasoby/NUL.txt", b"x".to_vec())]),
        ),
        ("zip-bomb", with(&[("zasoby/zera.bin", bomb)])),
        ("duplikat nazwy", with(&[("Version.JSON", b"{}".to_vec())])),
        ("dowiązanie", raw_zip(&base("1.1.0"), Some("link"))),
        (
            "brak version.json",
            raw_zip(&[("alfa-desktop.exe", b"MZ".to_vec())], None),
        ),
        ("niezgodny version.json", raw_zip(&base("9.9.9"), None)),
        ("brak aplikacji", raw_zip(&base("1.1.0")[1..], None)),
        ("nie ZIP", noise(4096, 9)),
    ];
    for (name, bytes) in cases {
        let f = Flow::new("1.0.0").await;
        let release = f.signed("1.1.0", "1.1.0", &bytes);
        let err = try_install(&f, release, bytes).await;
        assert!(
            matches!(err, UpdaterError::UnsafePackage { .. }),
            "{name}: {err:?}"
        );
        assert_untouched(&f, "1.1.0");
        let outside = f.updater.layout().versions.join("poza.txt");
        assert!(!outside.exists() && !f.updater.layout().root.join("poza.txt").exists());
    }
}
