//! Regresje z przeglądu bezpieczeństwa 2026-10 (SR-05): ostatnia linia obrony (`WinFs`, ścieżka
//! kanoniczna po rozwiązaniu dowiązań/junctions) zna całą bazową deny-listę Jądra z `compliance`,
//! nie tylko znaczniki kontraktu platformy — dowiązanie w profilu nie omija deny-listy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use compliance_contract::DenyLists;
use platform_windows_impl::{FsConfig, WinFs};

fn fs_in(dir: &Path) -> WinFs {
    WinFs::new(FsConfig {
        undo_dir: Some(dir.join("undo-base")),
        ..FsConfig::default()
    })
}

/// Każdy segment bazowej deny-listy Jądra jest odrzucany także przez `WinFs`.
#[test]
fn every_kernel_segment_is_denied_by_last_line() {
    let dir = tempfile::tempdir().unwrap();
    let fs = fs_in(dir.path());
    for seg in DenyLists::baseline().path_segments {
        let p = dir.path().join(&seg).join("x");
        assert!(fs.is_denied(&p), "{seg}");
        let upper = dir.path().join(seg.to_uppercase());
        assert!(fs.is_denied(&upper), "{seg} (wielkie litery)");
    }
}

/// Dowiązanie w zwykłym katalogu do magazynu tokenów innego CLI (lista Jądra, nie kontraktu
/// platformy) — odczyt i zapis przez dowiązanie są odrzucane po rozwiązaniu ścieżki.
#[cfg(unix)]
#[test]
fn symlink_into_kernel_denylisted_store_is_denied() {
    use platform_contract::{FsPort, PlatformError};
    let dir = tempfile::tempdir().unwrap();
    let fs = fs_in(dir.path());
    let store = dir.path().join(".gemini");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(store.join("oauth_creds.json"), b"sekret").unwrap();
    let link = dir.path().join("projekt");
    std::os::unix::fs::symlink(&store, &link).unwrap();
    let via = link.join("oauth_creds.json");
    assert!(matches!(fs.read(&via), Err(PlatformError::Denylisted(_))));
    assert!(matches!(
        fs.write_atomic(&via, b"x"),
        Err(PlatformError::Denylisted(_))
    ));
    let cfg = dir.path().join(".npmrc");
    std::fs::write(&cfg, b"//registry/:_authToken=x").unwrap();
    let file_link = dir.path().join("notatki.txt");
    std::os::unix::fs::symlink(&cfg, &file_link).unwrap();
    assert!(matches!(
        fs.read(&file_link),
        Err(PlatformError::Denylisted(_))
    ));
    std::fs::write(dir.path().join("zwykly.txt"), b"ok").unwrap();
    assert_eq!(fs.read(&dir.path().join("zwykly.txt")).unwrap(), b"ok");
}
