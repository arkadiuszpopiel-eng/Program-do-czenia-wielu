//! Bezpieczne rozpakowanie: 13 złośliwych archiwów (path traversal, ścieżka bezwzględna,
//! `\`, strumień NTFS, nazwa urządzenia, kropka na końcu, dowiązanie, duplikat różniący się
//! wielkością liter, zip-bomb, za dużo wpisów, za duży wpis, za dużo łącznie, zły hash wpisu)
//! — żaden nie zostawia plików poza celem, a poprzednia instalacja zostaje nietknięta;
//! poprawne archiwa: drzewo bez prefiksu i wybrane wpisy (paczka PyPI) z przypiętym hashem.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::path::Path;

use app_models::catalog::Pick;
use app_models::unpack::{extract_picks, extract_tree, staging_dir};
use sha2::{Digest, Sha256};
use updater_contract::PackageLimits;
use zip::write::SimpleFileOptions;

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

enum Entry<'a> {
    File(&'a str, &'a [u8]),
    Deflated(&'a str, Vec<u8>),
    Link(&'a str, &'a str),
}

fn archive(dir: &Path, name: &str, entries: &[Entry<'_>]) -> std::path::PathBuf {
    let path = dir.join(name);
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for e in entries {
        match e {
            Entry::File(n, b) => {
                zip.start_file(*n, stored).unwrap();
                zip.write_all(b).unwrap();
            }
            Entry::Deflated(n, b) => {
                zip.start_file(*n, SimpleFileOptions::default()).unwrap();
                zip.write_all(b).unwrap();
            }
            Entry::Link(n, to) => zip.add_symlink(*n, *to, stored).unwrap(),
        }
    }
    zip.finish().unwrap();
    path
}

fn tight() -> PackageLimits {
    PackageLimits {
        max_entries: 8,
        max_entry_bytes: 64 * 1024,
        max_total_bytes: 96 * 1024,
        max_ratio: 200,
    }
}

/// Wszystkie pliki pod `root` (względnie).
fn tree(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy();
                out.push(rel.replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn malicious_archives_are_rejected_without_side_effects() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sidecars");
    let target = root.join("engine");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("engine"), b"poprzednia wersja").unwrap();
    let bomb = vec![0u8; 2 * 1024 * 1024];
    let many: Vec<String> = (0..9).map(|i| format!("f{i}.txt")).collect();
    let big = vec![7u8; 65 * 1024];
    let half = vec![5u8; 50 * 1024];
    let cases: Vec<(&str, Vec<Entry<'_>>, PackageLimits)> = vec![
        (
            "traversal",
            vec![Entry::File("engine", b"x"), Entry::File("../../evil", b"x")],
            tight(),
        ),
        ("absolute", vec![Entry::File("/etc/evil", b"x")], tight()),
        ("backslash", vec![Entry::File("..\\evil", b"x")], tight()),
        (
            "ads",
            vec![Entry::File("engine:Zone.Identifier", b"x")],
            tight(),
        ),
        ("device", vec![Entry::File("lib/CON.txt", b"x")], tight()),
        (
            "trailing-dot",
            vec![Entry::File("engine.exe.", b"x")],
            tight(),
        ),
        (
            "symlink",
            vec![Entry::Link("engine", "/etc/passwd")],
            tight(),
        ),
        (
            "case-dup",
            vec![Entry::File("Engine", b"a"), Entry::File("engine", b"b")],
            tight(),
        ),
        (
            "bomb",
            vec![Entry::Deflated("engine", bomb)],
            PackageLimits::default(),
        ),
        (
            "many",
            many.iter().map(|n| Entry::File(n.as_str(), b"x")).collect(),
            tight(),
        ),
        ("big-entry", vec![Entry::File("engine", &big)], tight()),
        (
            "big-total",
            vec![Entry::File("engine", &half), Entry::File("lib.dll", &half)],
            tight(),
        ),
    ];
    for (name, entries, limits) in cases {
        let zip = archive(dir.path(), &format!("{name}.zip"), &entries);
        let result = extract_tree(&zip, &target, "", &["engine".to_owned()], limits);
        assert!(result.is_err(), "{name}: przyjęte");
        assert_eq!(tree(&root), ["engine/engine"], "{name}: skutki uboczne");
        assert_eq!(
            std::fs::read(target.join("engine")).unwrap(),
            b"poprzednia wersja",
            "{name}"
        );
        assert!(
            !staging_dir(&target).exists(),
            "{name}: katalog roboczy został"
        );
        assert!(!dir.path().join("evil").exists(), "{name}");
    }
    // Wpis wybrany z archiwum z innym hashem niż przypięty — odrzucony, bez pliku.
    let zip = archive(
        dir.path(),
        "pick.zip",
        &[Entry::File("pkg/data/m.onnx", b"model")],
    );
    let picks = [Pick {
        member: "pkg/data/m.onnx".into(),
        dest: "m.onnx".into(),
        sha256: Some("0".repeat(64)),
    }];
    let picked = dir.path().join("models");
    std::fs::create_dir_all(&picked).unwrap();
    assert!(extract_picks(&zip, &picked, &picks, tight()).is_err());
    assert!(tree(&picked).is_empty());
}

#[test]
fn valid_archives_extract_tree_and_picks() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("sidecars").join("whisper");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("stary.dll"), b"stary").unwrap();
    let zip = archive(
        dir.path(),
        "w.zip",
        &[
            Entry::File("Release/whisper-server", b"srv"),
            Entry::File("Release/ggml.dll", b"dll"),
            Entry::File("LICENSE", b"MIT"),
        ],
    );
    let require = ["whisper-server".to_owned()];
    let hashes = extract_tree(&zip, &target, "Release/", &require, tight()).unwrap();
    assert_eq!(
        tree(&target),
        ["ggml.dll", "whisper-server"],
        "stara wersja zastąpiona w całości"
    );
    assert_eq!(hashes.get("ggml.dll"), Some(&sha(b"dll")));
    // Brak wymaganego pliku (inny układ archiwum) → błąd, poprzednia instalacja zostaje.
    let other = archive(
        dir.path(),
        "o.zip",
        &[Entry::File("bin/whisper-server", b"srv")],
    );
    let err = extract_tree(&other, &target, "Release/", &require, tight()).unwrap_err();
    assert!(err.to_string().contains("do potwierdzenia"), "{err}");
    assert_eq!(tree(&target), ["ggml.dll", "whisper-server"]);
    // Paczka PyPI: wybrany wpis z przypiętym hashem pod nową nazwą; reszta pominięta.
    let wheel = archive(
        dir.path(),
        "pkg.whl",
        &[
            Entry::File("silero_vad/data/silero_vad.onnx", b"stary-format"),
            Entry::File("silero_vad/data/silero_vad_op18_ifless.onnx", b"model"),
        ],
    );
    let picks = [Pick {
        member: "silero_vad/data/silero_vad_op18_ifless.onnx".into(),
        dest: "silero_vad.onnx".into(),
        sha256: Some(sha(b"model")),
    }];
    let models = dir.path().join("models").join("silero");
    std::fs::create_dir_all(&models).unwrap();
    let got = extract_picks(&wheel, &models, &picks, tight()).unwrap();
    assert_eq!(got.get("silero_vad.onnx"), Some(&sha(b"model")));
    assert_eq!(
        std::fs::read(models.join("silero_vad.onnx")).unwrap(),
        b"model"
    );
    assert_eq!(tree(&models), ["silero_vad.onnx"]);
}
