//! R-13 (docs/STATUS.md): cofnięcie po restarcie aplikacji usuwa katalogi utworzone przez cofaną
//! operację. Token platformy z poprzedniego uruchomienia nie istnieje, więc dziennik odtwarza
//! stan z własnych wpisów; `FakeFs` ma katalogi domniemane, dlatego test używa systemu plików
//! z jawnymi katalogami.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use platform_contract::{
    DirEntry, FsOperation, FsPort, KnownFolder, OpReceipt, PlatformError, UndoToken,
};
use undo_journal_contract::{StepCtx, UndoJournal, UndoLimits};
use undo_journal_impl::UndoService;

#[derive(Default)]
struct Tree {
    files: BTreeMap<PathBuf, Vec<u8>>,
    dirs: BTreeSet<PathBuf>,
}

/// System plików w pamięci z jawnymi katalogami; cofanie tokenem zawsze odmawia.
#[derive(Default)]
struct DirFs(Mutex<Tree>);

impl DirFs {
    fn with_dir(dir: &str) -> Arc<Self> {
        let fs = Self::default();
        fs.0.lock().unwrap().dirs.insert(PathBuf::from(dir));
        Arc::new(fs)
    }

    fn has_dir(&self, dir: &str) -> bool {
        self.0.lock().unwrap().dirs.contains(Path::new(dir))
    }

    fn put(&self, path: &str, data: &[u8]) {
        let mut tree = self.0.lock().unwrap();
        make_parents(&mut tree, Path::new(path));
        tree.files.insert(PathBuf::from(path), data.to_vec());
    }
}

fn make_parents(tree: &mut Tree, path: &Path) {
    for dir in path.ancestors().skip(1) {
        if !dir.as_os_str().is_empty() {
            tree.dirs.insert(dir.to_path_buf());
        }
    }
}

fn receipt(op: FsOperation) -> OpReceipt {
    OpReceipt::reversible(op, UndoToken(1))
}

impl FsPort for DirFs {
    fn read(&self, path: &Path) -> Result<Vec<u8>, PlatformError> {
        let tree = self.0.lock().unwrap();
        tree.files
            .get(path)
            .cloned()
            .ok_or_else(|| PlatformError::NotFound(path.to_path_buf()))
    }
    fn write_atomic(&self, path: &Path, data: &[u8]) -> Result<OpReceipt, PlatformError> {
        let mut tree = self.0.lock().unwrap();
        make_parents(&mut tree, path);
        tree.files.insert(path.to_path_buf(), data.to_vec());
        Ok(receipt(FsOperation::WriteAtomic))
    }
    fn copy(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError> {
        let data = self.read(from)?;
        self.write_atomic(to, &data)?;
        Ok(receipt(FsOperation::Copy))
    }
    fn move_path(&self, from: &Path, to: &Path) -> Result<OpReceipt, PlatformError> {
        let data = self.read(from)?;
        self.write_atomic(to, &data)?;
        self.0.lock().unwrap().files.remove(from);
        Ok(receipt(FsOperation::Move))
    }
    fn delete_to_recycle_bin(&self, path: &Path) -> Result<OpReceipt, PlatformError> {
        let mut tree = self.0.lock().unwrap();
        if tree.files.remove(path).is_some() || tree.dirs.remove(path) {
            return Ok(receipt(FsOperation::DeleteToRecycleBin));
        }
        Err(PlatformError::NotFound(path.to_path_buf()))
    }
    fn delete_permanent(&self, path: &Path) -> Result<OpReceipt, PlatformError> {
        self.delete_to_recycle_bin(path)?;
        Ok(OpReceipt::irreversible(FsOperation::DeletePermanent))
    }
    fn exists(&self, path: &Path) -> bool {
        let tree = self.0.lock().unwrap();
        tree.files.contains_key(path) || tree.dirs.contains(path)
    }
    fn list_dir(&self, path: &Path) -> Result<Vec<DirEntry>, PlatformError> {
        let tree = self.0.lock().unwrap();
        if !tree.dirs.contains(path) {
            return Err(PlatformError::NotFound(path.to_path_buf()));
        }
        let child = |p: &PathBuf| p.parent() == Some(path);
        let files = tree
            .files
            .iter()
            .filter(|(p, _)| child(p))
            .map(|(p, d)| DirEntry {
                path: p.clone(),
                is_dir: false,
                size: d.len() as u64,
            });
        let dirs = tree.dirs.iter().filter(|p| child(p)).map(|p| DirEntry {
            path: p.clone(),
            is_dir: true,
            size: 0,
        });
        Ok(files.chain(dirs).collect())
    }
    fn undo(&self, token: UndoToken) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported(format!("token {}", token.0)))
    }
    fn known_folder(&self, _: KnownFolder) -> PathBuf {
        PathBuf::from("/")
    }
}

fn open(fs: &Arc<DirFs>, dir: &Path, boot: u64) -> UndoService {
    UndoService::open(
        fs.clone(),
        dir,
        UndoLimits::default(),
        Arc::new(|| 5_000u64),
        boot,
    )
    .unwrap()
}

#[test]
fn undo_after_restart_removes_directories_the_step_created() {
    let store = tempfile::tempdir().unwrap();
    let fs = DirFs::with_dir("/w");
    let s = open(&fs, store.path(), 1);
    let step = s
        .begin_step(StepCtx::new("s1", Some("Delta"), "x"))
        .unwrap();
    s.write(step, Path::new("/w/nowy/gleboko/plik.txt"), b"tresc")
        .unwrap();
    s.commit_step(step).unwrap();
    assert!(fs.has_dir("/w/nowy/gleboko"));
    drop(s);

    let s = open(&fs, store.path(), 2);
    s.undo(step).unwrap();
    assert!(!fs.exists(Path::new("/w/nowy/gleboko/plik.txt")));
    assert!(!fs.has_dir("/w/nowy/gleboko"), "pusty katalog ma zniknąć");
    assert!(!fs.has_dir("/w/nowy"), "pusty katalog nadrzędny też");
    assert!(fs.has_dir("/w"), "katalog sprzed operacji zostaje");
}

#[test]
fn undo_after_restart_keeps_directories_with_foreign_content() {
    let store = tempfile::tempdir().unwrap();
    let fs = DirFs::with_dir("/w");
    let s = open(&fs, store.path(), 1);
    let step = s
        .begin_step(StepCtx::new("s1", Some("Delta"), "x"))
        .unwrap();
    s.write(step, Path::new("/w/nowy/gleboko/plik.txt"), b"tresc")
        .unwrap();
    s.commit_step(step).unwrap();
    drop(s);
    // Właściciel dołożył w międzyczasie własny plik do katalogu utworzonego przez agentkę.
    fs.put("/w/nowy/moje.txt", b"moje");

    let s = open(&fs, store.path(), 2);
    s.undo(step).unwrap();
    assert!(!fs.has_dir("/w/nowy/gleboko"));
    assert!(fs.has_dir("/w/nowy"), "katalog z cudzą treścią zostaje");
    assert_eq!(fs.read(Path::new("/w/nowy/moje.txt")).unwrap(), b"moje");
}
