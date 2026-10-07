//! Testy `WinFs` na prawdziwym katalogu tymczasowym (każdy OS; Kosz tylko na Windows).
//! Te same reguły co w `platform-fake`: odwracalność, jednorazowe tokeny, deny-lista.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use platform_contract::{FsOperation, FsPort, KnownFolder, OpReceipt, PlatformError};
use platform_windows_impl::{FsConfig, WinFs};
use proptest::prelude::*;

fn setup() -> (tempfile::TempDir, WinFs) {
    let dir = tempfile::tempdir().unwrap();
    let fs = WinFs::new(FsConfig {
        undo_dir: Some(dir.path().join("undo-base")),
        ..FsConfig::default()
    });
    (dir, fs)
}

/// Migawka drzewa (pliki z treścią, katalogi z `None`), bez katalogu kopii zapasowych.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path.strip_prefix(root).unwrap().to_path_buf();
            if rel.starts_with("undo-base") {
                continue;
            }
            if path.is_dir() {
                out.insert(rel, None);
                stack.push(path);
            } else {
                out.insert(rel, Some(fs::read(&path).unwrap()));
            }
        }
    }
    out
}

#[test]
fn write_atomic_is_reversible_for_new_and_existing_files() {
    let (dir, fs) = setup();
    let file = dir.path().join("a").join("b").join("note.txt");
    let r1 = fs.write_atomic(&file, b"pierwsza").unwrap();
    assert_eq!(r1.op, FsOperation::WriteAtomic);
    assert!(r1.reversible && r1.undo.is_some());
    assert_eq!(fs.read(&file).unwrap(), b"pierwsza");
    let r2 = fs.write_atomic(&file, b"druga").unwrap();
    assert_eq!(fs.read(&file).unwrap(), b"druga");
    fs.undo(r2.undo.unwrap()).unwrap();
    assert_eq!(fs.read(&file).unwrap(), b"pierwsza");
    fs.undo(r1.undo.unwrap()).unwrap();
    assert!(!fs.exists(&file));
    assert!(
        !dir.path().join("a").exists(),
        "utworzone katalogi usunięte"
    );
    assert_eq!(
        fs.undo(r1.undo.unwrap()),
        Err(PlatformError::UnknownUndoToken(r1.undo.unwrap().0))
    );
    assert_eq!(fs.pending_undo(), 0);
}

#[test]
fn undo_refuses_to_clobber_later_changes() {
    let (dir, fs) = setup();
    let file = dir.path().join("f.txt");
    let receipt = fs.write_atomic(&file, b"v1").unwrap();
    fs::write(&file, b"zmienione przez kogos innego, dluzsze").unwrap();
    let token = receipt.undo.unwrap();
    assert!(matches!(
        fs.undo(token),
        Err(PlatformError::NotReversible(_))
    ));
    assert_eq!(
        fs.pending_undo(),
        1,
        "token wraca do dziennika po nieudanym cofnięciu"
    );
}

#[test]
fn copy_and_move_files_and_trees() {
    let (dir, fs) = setup();
    let src = dir.path().join("src.bin");
    fs::write(&src, b"dane").unwrap();
    let copy = fs
        .copy(&src, &dir.path().join("kopie").join("c.bin"))
        .unwrap();
    assert_eq!(
        fs.read(&dir.path().join("kopie").join("c.bin")).unwrap(),
        b"dane"
    );
    assert_eq!(
        fs.copy(&src, &dir.path().join("kopie").join("c.bin")),
        Err(PlatformError::AlreadyExists(
            dir.path().join("kopie").join("c.bin")
        ))
    );
    assert_eq!(
        fs.copy(&dir.path().join("brak"), &dir.path().join("x")),
        Err(PlatformError::NotFound(dir.path().join("brak")))
    );
    fs.undo(copy.undo.unwrap()).unwrap();
    assert!(!dir.path().join("kopie").exists());

    let tree = dir.path().join("drzewo");
    fs::create_dir_all(tree.join("pod")).unwrap();
    fs::write(tree.join("pod").join("x.txt"), b"x").unwrap();
    let copied = fs.copy(&tree, &dir.path().join("drzewo2")).unwrap();
    assert_eq!(
        fs.read(&dir.path().join("drzewo2").join("pod").join("x.txt"))
            .unwrap(),
        b"x"
    );
    fs.undo(copied.undo.unwrap()).unwrap();
    assert!(!dir.path().join("drzewo2").exists());
    assert!(matches!(
        fs.copy(&tree, &tree.join("pod").join("w-sobie")),
        Err(PlatformError::InvalidPath(_))
    ));

    let moved = fs
        .move_path(&src, &dir.path().join("m").join("src.bin"))
        .unwrap();
    assert!(!src.exists());
    fs.undo(moved.undo.unwrap()).unwrap();
    assert_eq!(fs::read(&src).unwrap(), b"dane");
    fs::write(dir.path().join("zajete"), b"z").unwrap();
    assert!(matches!(
        fs.move_path(&src, &dir.path().join("zajete")),
        Err(PlatformError::AlreadyExists(_))
    ));
    let same = fs.move_path(&src, &src).unwrap();
    fs.undo(same.undo.unwrap()).unwrap();
    assert!(src.exists());
}

#[test]
fn tree_copy_with_credentials_is_refused_without_leftovers() {
    let (dir, fs) = setup();
    let tree = dir.path().join("projekt");
    fs::create_dir_all(tree.join(".claude")).unwrap();
    fs::write(tree.join(".claude").join("settings.json"), b"{}").unwrap();
    fs::write(tree.join("main.rs"), b"fn main() {}").unwrap();
    let target = dir.path().join("kopia");
    assert!(matches!(
        fs.copy(&tree, &target),
        Err(PlatformError::Denylisted(_))
    ));
    assert!(!target.exists());
}

#[test]
fn denylist_is_enforced_for_every_operation() {
    let (dir, fs) = setup();
    let secret_dir = dir.path().join(".codex");
    fs::create_dir(&secret_dir).unwrap();
    let secret = secret_dir.join("auth.json");
    fs::write(&secret, b"token").unwrap();
    let other = dir.path().join("x.txt");
    fs::write(&other, b"x").unwrap();
    let denied =
        |r: Result<OpReceipt, PlatformError>| matches!(r, Err(PlatformError::Denylisted(_)));
    assert!(matches!(
        fs.read(&secret),
        Err(PlatformError::Denylisted(_))
    ));
    assert!(denied(fs.write_atomic(&secret, b"y")));
    assert!(denied(fs.copy(&secret, &dir.path().join("leak"))));
    assert!(denied(fs.copy(&other, &secret_dir.join("plant"))));
    assert!(denied(fs.move_path(&secret, &dir.path().join("leak"))));
    assert!(denied(fs.delete_permanent(&secret)));
    assert!(denied(fs.delete_to_recycle_bin(&secret)));
    assert!(matches!(
        fs.list_dir(&secret_dir),
        Err(PlatformError::Denylisted(_))
    ));
    assert!(!fs.exists(&secret));
    assert!(fs.is_denied(&secret));
    let upper = dir.path().join(".CODEX.").join("auth.json");
    assert!(matches!(fs.read(&upper), Err(PlatformError::Denylisted(_))));
    assert_eq!(fs::read(&secret).unwrap(), b"token");
}

#[test]
fn delete_list_and_known_folders() {
    let (dir, fs) = setup();
    fs::create_dir_all(dir.path().join("d").join("e")).unwrap();
    fs::write(dir.path().join("d").join("b.txt"), b"bb").unwrap();
    fs::write(dir.path().join("d").join("a.txt"), b"a").unwrap();
    let entries = fs.list_dir(&dir.path().join("d")).unwrap();
    let names: Vec<_> = entries
        .iter()
        .map(|e| e.path.file_name().unwrap().to_owned())
        .collect();
    assert_eq!(names, ["a.txt", "b.txt", "e"]);
    assert_eq!((entries[1].size, entries[2].is_dir), (2, true));
    assert!(matches!(
        fs.list_dir(&dir.path().join("d").join("a.txt")),
        Err(PlatformError::InvalidPath(_))
    ));
    assert!(matches!(
        fs.list_dir(&dir.path().join("nie")),
        Err(PlatformError::NotFound(_))
    ));
    let receipt = fs.delete_permanent(&dir.path().join("d")).unwrap();
    assert_eq!(
        receipt,
        OpReceipt::irreversible(FsOperation::DeletePermanent)
    );
    assert!(!dir.path().join("d").exists());
    assert!(matches!(
        fs.delete_permanent(&dir.path().join("d")),
        Err(PlatformError::NotFound(_))
    ));
    assert!(matches!(
        fs.read(Path::new("wzgledna.txt")),
        Err(PlatformError::InvalidPath(_))
    ));
    for folder in [
        KnownFolder::LocalAppData,
        KnownFolder::RoamingAppData,
        KnownFolder::Home,
        KnownFolder::Temp,
    ] {
        assert!(fs.known_folder(folder).is_absolute(), "{folder:?}");
    }
    if !cfg!(windows) {
        fs::write(dir.path().join("k"), b"k").unwrap();
        assert!(matches!(
            fs.delete_to_recycle_bin(&dir.path().join("k")),
            Err(PlatformError::Unsupported(_))
        ));
    }
}

#[derive(Debug, Clone)]
enum Op {
    Write(u8, Vec<u8>),
    Copy(u8, u8),
    Move(u8, u8),
}

fn rel(n: u8) -> PathBuf {
    PathBuf::from(format!("w{}", n % 3)).join(format!("f{n}.txt"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Dowolna sekwencja operacji cofnięta w kolejności LIFO przywraca drzewo co do bajtu.
    #[test]
    fn lifo_undo_restores_initial_tree(
        initial in prop::collection::btree_map(0..6u8, prop::collection::vec(any::<u8>(), 0..4), 0..4),
        ops in prop::collection::vec(prop_oneof![
            (0..6u8, prop::collection::vec(any::<u8>(), 0..6)).prop_map(|(p, d)| Op::Write(p, d)),
            (0..6u8, 0..6u8).prop_map(|(a, b)| Op::Copy(a, b)),
            (0..6u8, 0..6u8).prop_map(|(a, b)| Op::Move(a, b)),
        ], 1..10),
    ) {
        let (dir, fs) = setup();
        for (n, data) in &initial {
            let path = dir.path().join(rel(*n));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, data).unwrap();
        }
        let before = snapshot(dir.path());
        let mut tokens = Vec::new();
        for op in &ops {
            let at = |n: u8| dir.path().join(rel(n));
            let result = match op {
                Op::Write(p, d) => fs.write_atomic(&at(*p), d),
                Op::Copy(a, b) => fs.copy(&at(*a), &at(*b)),
                Op::Move(a, b) => fs.move_path(&at(*a), &at(*b)),
            };
            if let Ok(receipt) = result {
                prop_assert!(receipt.reversible);
                tokens.push(receipt.undo.unwrap());
            }
        }
        for token in tokens.into_iter().rev() {
            fs.undo(token).unwrap();
        }
        prop_assert_eq!(snapshot(dir.path()), before);
        prop_assert_eq!(fs.pending_undo(), 0);
    }
}
