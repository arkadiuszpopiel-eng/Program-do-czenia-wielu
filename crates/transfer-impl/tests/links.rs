//! Regresja klas ostrzeżeń `zip` RUSTSEC-2026-0367/0368/0369 (bity SUID/SGID przenoszone
//! z archiwum, dowiązania wskazujące poza katalog docelowy, prawa nadawane przez dowiązanie):
//! kontener `.alfa` odrzuca każdy wpis-dowiązanie (także z hostem DOS) bez żadnego zapisu,
//! a tryb pliku z archiwum nigdy nie trafia na dysk (wpisy czytane do pamięci, zapis przez
//! magazyn dokumentów).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::io::{Read, Write};
use std::path::Path;

use common::{craft, files_under, harness};
use transfer_contract::contract_tests::Harness;
use transfer_contract::{Category, DocumentStore, ImportOptions, Transfer, TransferError};
use zip::write::SimpleFileOptions;

/// Jak zapisać wpisy treści paczki.
enum Kind {
    /// Dowiązanie symboliczne (treść = cel dowiązania).
    Link,
    /// Dowiązanie z hostem DOS (`S_IFLNK` w górnych bitach atrybutów) — `add_symlink` zawsze
    /// zapisuje host Unix, a zip 9 nie czyta trybu Unix z wpisów DOS.
    DosLink,
    /// Plik z pełnym trybem Unix (także bity specjalne) w atrybutach zewnętrznych.
    Mode(u32),
}

/// Paczka jak z [`craft`] (manifest v1 wyliczony z treści), ale wpisy zapisane jako `kind`.
fn rebuild(path: &Path, entries: &[(&str, &[u8])], kind: &Kind) {
    let owned: Vec<(&str, Vec<u8>)> = entries.iter().map(|(n, b)| (*n, b.to_vec())).collect();
    craft(path, None, &owned);
    let mut manifest = Vec::new();
    zip::ZipArchive::new(std::fs::File::open(path).unwrap())
        .unwrap()
        .by_index(0)
        .unwrap()
        .read_to_end(&mut manifest)
        .unwrap();
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("manifest.json", stored).unwrap();
    zip.write_all(&manifest).unwrap();
    for (name, bytes) in entries {
        match kind {
            Kind::Link => {
                let to = std::str::from_utf8(bytes).unwrap();
                zip.add_symlink(*name, to, stored).unwrap();
            }
            Kind::DosLink => {
                let dos = stored
                    .system(zip::System::Dos)
                    .external_attributes(0o120_777 << 16);
                zip.start_file(*name, dos).unwrap();
                zip.write_all(bytes).unwrap();
            }
            Kind::Mode(mode) => {
                let unix = stored
                    .system(zip::System::Unix)
                    .external_attributes(mode << 16);
                zip.start_file(*name, unix).unwrap();
                zip.write_all(bytes).unwrap();
            }
        }
    }
    zip.finish().unwrap();
}

/// Pliki pod katalogiem testu poza paczkami.
fn written(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = files_under(root);
    out.retain(|p| !p.starts_with("paczki"));
    out
}

#[test]
fn link_entries_are_rejected_without_writes() {
    let h = harness();
    let outside = h.dir.path().join("poza");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("tajne.toml"), b"klucz = 1\n").unwrap();
    let before = written(h.dir.path());
    let absolute = outside.join("tajne.toml").to_string_lossy().into_owned();
    let targets = [absolute.as_str(), "../../poza/tajne.toml", "shared.toml"];
    let cases = targets
        .iter()
        .map(|to| (*to, Kind::Link))
        .chain(targets.iter().map(|to| (*to, Kind::DosLink)));
    for (i, (to, kind)) in cases.enumerate() {
        let pkg = h.path(&format!("dowiazanie-{i}.alfa"));
        rebuild(&pkg, &[("config/common/a.toml", to.as_bytes())], &kind);
        let err = h
            .transfer
            .import(&pkg, &ImportOptions::default())
            .unwrap_err();
        assert!(
            matches!(err, TransferError::Corrupt { .. }),
            "{to}: {err:?}"
        );
        assert!(h.transfer.inspect(&pkg, &ImportOptions::default()).is_err());
        assert_eq!(written(h.dir.path()), before, "{to}: zapis mimo błędu");
    }
    assert_eq!(
        std::fs::read(outside.join("tajne.toml")).unwrap(),
        b"klucz = 1\n"
    );
}

#[test]
fn special_mode_bits_never_reach_disk() {
    let h = harness();
    let pkg = h.path("suid.alfa");
    // S_IFREG | SUID | SGID | sticky | 0o755.
    rebuild(
        &pkg,
        &[("config/common/a.toml", b"a = 1\n")],
        &Kind::Mode(0o107_755),
    );
    h.transfer.import(&pkg, &ImportOptions::default()).unwrap();
    assert_eq!(
        h.stores[&Category::ConfigCommon].read("a.toml").unwrap(),
        Some(b"a = 1\n".to_vec())
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file = h
            .dir
            .path()
            .join("dane")
            .join(Category::ConfigCommon.dir())
            .join("a.toml");
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o7000, 0, "{mode:o}");
    }
}
