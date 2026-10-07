//! Odporność kontenera: uszkodzony plik, zła suma, obcięte archiwum, path traversal (`../`,
//! ścieżki bezwzględne, `C:\`, UNC), zip-bomb (limity) → czytelny błąd i zero zapisów.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{craft, files_under, harness, harness_with};
use proptest::prelude::*;
use transfer_contract::contract_tests::{Harness, seed};
use transfer_contract::{
    ExportRequest, ExportScope, ImportMode, ImportOptions, Limits, ModeMap, Selection, Transfer,
    TransferError,
};

fn opts_replace() -> ImportOptions {
    ImportOptions {
        modes: ModeMap::all(ImportMode::Replace),
        ..ImportOptions::default()
    }
}

#[test]
fn manifest_first_stored_and_encrypted_file_is_opaque() {
    let h = harness();
    seed(&h, false);
    let plain = h.path("jawna.alfa");
    h.transfer
        .export(&ExportRequest::new(ExportScope::default(), &plain))
        .unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&plain).unwrap()).unwrap();
    let first = zip.by_index(0).unwrap();
    assert_eq!(first.name(), "manifest.json");
    assert_eq!(first.compression(), zip::CompressionMethod::Stored);
    drop(first);
    let enc = h.path("szyfr.alfa");
    let mut req = ExportRequest::new(ExportScope::default(), &enc);
    req.password = Some("hasło testowe 123".into());
    h.transfer.export(&req).unwrap();
    let bytes = std::fs::read(&enc).unwrap();
    assert!(bytes.starts_with(b"ALFAENC1"));
    assert!(!bytes.windows(13).any(|w| w == b"manifest.json"));
    assert!(!bytes.windows(5).any(|w| w == b"piper"));
}

#[test]
fn damaged_packages_give_clear_errors_and_no_writes() {
    let h = harness();
    seed(&h, false);
    let pkg = h.path("dobra.alfa");
    let scope = ExportScope {
        sessions: Selection::All,
        ..ExportScope::default()
    };
    h.transfer.export(&ExportRequest::new(scope, &pkg)).unwrap();
    let good = std::fs::read(&pkg).unwrap();
    let data = h.dir.path().join("dane");
    let before = files_under(&data);
    let check = |bytes: &[u8], name: &str| {
        let p = h.path(name);
        std::fs::write(&p, bytes).unwrap();
        let err = h.transfer.import(&p, &opts_replace()).unwrap_err();
        assert!(
            matches!(
                err,
                TransferError::Corrupt { .. } | TransferError::Checksum { .. }
            ),
            "{name}: {err:?}"
        );
        assert!(!err.to_string().is_empty());
        assert_eq!(files_under(&data), before, "{name}: zapis mimo błędu");
    };
    check(b"to nie jest zip", "smiec.alfa");
    check(&good[..good.len() / 2], "obcieta.alfa");
    check(&good[..good.len() - 10], "bez-konca.alfa");
    let mut flipped = good.clone();
    let mid = flipped.len() / 2;
    flipped[mid] ^= 0x55;
    let p = h.path("przestawiona.alfa");
    std::fs::write(&p, &flipped).unwrap();
    assert!(h.transfer.import(&p, &opts_replace()).is_err());
    assert!(h.transfer.snapshots().unwrap().is_empty());

    // Zła suma w manifeście.
    let mut manifest: serde_json::Value = {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(good.clone())).unwrap();
        serde_json::from_reader(zip.by_index(0).unwrap()).unwrap()
    };
    let doc = h.path("zla-suma.alfa");
    manifest["content"][0]["sha256"] = serde_json::json!("0".repeat(64));
    manifest["content_sha256"] = serde_json::json!(transfer_contract::content_sha256(
        &serde_json::from_value::<Vec<transfer_contract::ContentEntry>>(
            manifest["content"].clone()
        )
        .unwrap()
    ));
    let entries: Vec<(String, Vec<u8>)> = h.entries(&pkg).into_iter().skip(1).collect();
    let refs: Vec<(&str, Vec<u8>)> = entries
        .iter()
        .map(|(n, b)| (n.as_str(), b.clone()))
        .collect();
    craft(&doc, Some(manifest), &refs);
    assert!(matches!(
        h.transfer.import(&doc, &opts_replace()),
        Err(TransferError::Checksum { .. })
    ));
    assert_eq!(files_under(&data), before);

    // Pierwszy wpis nie jest manifestem / wpis spoza manifestu.
    let odd = h.path("kolejnosc.alfa");
    {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&odd).unwrap());
        zip.start_file("a.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"x").unwrap();
        zip.finish().unwrap();
    }
    assert!(matches!(
        h.transfer.inspect(&odd, &ImportOptions::default()),
        Err(TransferError::Corrupt { .. })
    ));
    let extra = h.path("nadmiarowy.alfa");
    let base = serde_json::json!(null);
    drop(base);
    craft(
        &extra,
        None,
        &[("config/common/a.toml", b"a = 1\n".to_vec())],
    );
    let mut m: serde_json::Value = {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&extra).unwrap()).unwrap();
        serde_json::from_reader(zip.by_index(0).unwrap()).unwrap()
    };
    m["content"] = serde_json::json!([]);
    m["content_sha256"] = serde_json::json!(transfer_contract::content_sha256(&[]));
    craft(
        &extra,
        Some(m),
        &[("config/common/a.toml", b"a = 1\n".to_vec())],
    );
    assert!(matches!(
        h.transfer.inspect(&extra, &ImportOptions::default()),
        Err(TransferError::Corrupt { .. })
    ));
}

#[test]
fn encrypted_damage_is_detected() {
    let h = harness();
    seed(&h, false);
    let pkg = h.path("szyfr.alfa");
    let mut req = ExportRequest::new(
        ExportScope {
            sessions: Selection::All,
            include_private: true,
            ..ExportScope::default()
        },
        &pkg,
    );
    req.password = Some("hasło testowe 123".into());
    h.transfer.export(&req).unwrap();
    let good = std::fs::read(&pkg).unwrap();
    let opts = ImportOptions {
        password: Some("hasło testowe 123".into()),
        ..ImportOptions::default()
    };
    for (name, bytes) in [
        ("obciety", good[..good.len() - 7].to_vec()),
        ("bez-ostatniego", good[..good.len() - 16].to_vec()),
        ("bit", {
            let mut b = good.clone();
            let n = b.len();
            b[n - 3] ^= 1;
            b
        }),
        ("naglowek", {
            let mut b = good.clone();
            b[20] ^= 1;
            b
        }),
    ] {
        let p = h.path(&format!("{name}.alfa"));
        std::fs::write(&p, bytes).unwrap();
        let err = h.transfer.inspect(&p, &opts).unwrap_err();
        assert!(
            matches!(
                err,
                TransferError::Corrupt { .. }
                    | TransferError::WrongPassword
                    | TransferError::Checksum { .. }
            ),
            "{name}: {err:?}"
        );
    }
}

fn traversal_names() -> Vec<&'static str> {
    vec![
        "../evil.toml",
        "config/common/../../evil.toml",
        "/etc/evil.toml",
        "C:/Windows/evil.toml",
        "C:\\Windows\\evil.toml",
        "\\\\serwer\\udzial\\evil.toml",
        "//serwer/udzial/evil.toml",
        "config/common/..\\..\\evil.toml",
        "config/common/CON.toml",
        "config/common/evil.toml:ads",
        "config/common/evil.toml.",
    ]
}

#[test]
fn path_traversal_is_rejected_without_writes() {
    let h = harness();
    let outside = h.dir.path().to_path_buf();
    let before = files_under(&outside);
    for (i, name) in traversal_names().into_iter().enumerate() {
        let pkg = h.path(&format!("zly-{i}.alfa"));
        craft(&pkg, None, &[(name, b"a = 1\n".to_vec())]);
        let err = h.transfer.import(&pkg, &opts_replace()).unwrap_err();
        assert!(
            matches!(err, TransferError::UnsafePath { .. }),
            "{name}: {err:?}"
        );
        let mut after = files_under(&outside);
        after.retain(|p| !p.starts_with("paczki"));
        let mut expected = before.clone();
        expected.retain(|p| !p.starts_with("paczki"));
        assert_eq!(after, expected, "{name}: zapis poza katalogiem docelowym");
    }
}

#[test]
fn zip_bomb_limits() {
    let zeros = vec![0u8; 4 << 20];
    let h = harness_with(Limits {
        max_ratio: 50,
        ..Limits::default()
    });
    let pkg = h.path("bomba.alfa");
    craft(&pkg, None, &[("logs/zera.ndjson", zeros.clone())]);
    assert!(matches!(
        h.transfer.inspect(&pkg, &ImportOptions::default()),
        Err(TransferError::LimitExceeded { .. })
    ));

    let h = harness_with(Limits {
        max_total_bytes: 1 << 20,
        ..Limits::default()
    });
    let pkg = h.path("duza.alfa");
    craft(&pkg, None, &[("logs/zera.ndjson", zeros)]);
    assert!(matches!(
        h.transfer.inspect(&pkg, &ImportOptions::default()),
        Err(TransferError::LimitExceeded { .. })
    ));

    let h = harness_with(Limits {
        max_entries: 5,
        ..Limits::default()
    });
    let pkg = h.path("wiele.alfa");
    let many: Vec<(String, Vec<u8>)> = (0..10)
        .map(|i| (format!("logs/{i}.ndjson"), b"{}\n".to_vec()))
        .collect();
    let refs: Vec<(&str, Vec<u8>)> = many.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    craft(&pkg, None, &refs);
    assert!(matches!(
        h.transfer.inspect(&pkg, &ImportOptions::default()),
        Err(TransferError::LimitExceeded { .. })
    ));

    let h = harness_with(Limits {
        max_manifest_bytes: 64,
        ..Limits::default()
    });
    let pkg = h.path("manifest.alfa");
    craft(&pkg, None, &[("logs/a.ndjson", b"{}\n".to_vec())]);
    assert!(matches!(
        h.transfer.inspect(&pkg, &ImportOptions::default()),
        Err(TransferError::LimitExceeded { .. })
    ));
}

#[test]
fn newer_schema_is_rejected_with_update_hint() {
    let h = harness();
    for version in ["2.0.0", "1.1.0"] {
        let pkg = h.path(&format!("nowa-{version}.alfa"));
        craft(&pkg, None, &[]);
        let mut m: serde_json::Value = {
            let mut zip = zip::ZipArchive::new(std::fs::File::open(&pkg).unwrap()).unwrap();
            serde_json::from_reader(zip.by_index(0).unwrap()).unwrap()
        };
        m["schema_version"] = serde_json::json!(version);
        craft(&pkg, Some(m), &[]);
        let err = h
            .transfer
            .inspect(&pkg, &ImportOptions::default())
            .unwrap_err();
        assert!(matches!(err, TransferError::NewerSchema { .. }), "{err:?}");
        assert!(err.to_string().contains("zaktualizuj Alfę"));
    }
}

#[test]
fn export_failure_leaves_no_files() {
    let h = harness();
    seed(&h, false);
    let pkg = h.path("nieudany.alfa");
    std::fs::create_dir_all(pkg.parent().unwrap()).unwrap();
    let scope = ExportScope {
        sessions: Selection::Only(vec![sessions_contract::SessionId::new("brak")]),
        ..ExportScope::default()
    };
    assert!(h.transfer.export(&ExportRequest::new(scope, &pkg)).is_err());
    assert!(
        std::fs::read_dir(pkg.parent().unwrap())
            .unwrap()
            .next()
            .is_none(),
        "pliki tymczasowe zostały"
    );
}

fn segment() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("..".to_owned()),
        Just(".".to_owned()),
        Just(String::new()),
        Just("C:".to_owned()),
        Just("\\\\srv".to_owned()),
        Just("CON".to_owned()),
        Just("a ".to_owned()),
        Just("config".to_owned()),
        Just("common".to_owned()),
        Just("x.toml".to_owned()),
        "[a-zA-Ząż0-9._\\-]{1,8}",
        "[\\x00-\\x7f]{1,4}",
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, failure_persistence: None, ..ProptestConfig::default() })]

    /// Dowolne nazwy wpisów: import albo odrzuca paczkę, albo zapisuje wyłącznie w katalogach
    /// magazynów (i snapshotów) — nigdy obok, wyżej ani pod ścieżką bezwzględną.
    #[test]
    fn arbitrary_entry_names_never_escape(segs in proptest::collection::vec(segment(), 1..5), sep in prop_oneof![Just("/"), Just("\\")]) {
        let h = harness();
        let name = segs.join(sep);
        let pkg = h.path("losowa.alfa");
        craft(&pkg, None, &[(name.as_str(), b"a = 1\n".to_vec()), (&format!("config/common/{name}"), b"b = 2\n".to_vec())]);
        let _ = h.transfer.import(&pkg, &opts_replace());
        for file in files_under(h.dir.path()) {
            let ok = file.starts_with("paczki") || file.starts_with("snapshots") || file.starts_with("dane");
            prop_assert!(ok, "plik poza katalogami: {:?} (nazwa {:?})", file, name);
            if let Ok(rel) = file.strip_prefix("dane") {
                prop_assert!(!rel.components().any(|c| matches!(c, std::path::Component::ParentDir)));
            }
        }
        prop_assert!(!std::path::Path::new("/evil.toml").exists());
    }
}
