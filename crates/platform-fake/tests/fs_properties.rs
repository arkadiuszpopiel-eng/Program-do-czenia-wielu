//! Testy właściwościowe wirtualnego FS: każda odwracalna operacja cofa się do migawki,
//! a cofnięcie całej sekwencji (LIFO) odtwarza stan początkowy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};

use platform_contract::{FsOperation, FsPort, OpReceipt, PlatformError};
use platform_fake::{FakeFs, FsSnapshot};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Write(u8, Vec<u8>),
    Copy(u8, u8),
    Move(u8, u8),
    Recycle(u8),
}

fn path(n: u8) -> PathBuf {
    PathBuf::from(format!("/work/{}/f{}.txt", n % 3, n))
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..6u8, prop::collection::vec(any::<u8>(), 0..8)).prop_map(|(p, d)| Op::Write(p, d)),
        (0..6u8, 0..6u8).prop_map(|(a, b)| Op::Copy(a, b)),
        (0..6u8, 0..6u8).prop_map(|(a, b)| Op::Move(a, b)),
        (0..6u8).prop_map(Op::Recycle),
    ]
}

fn initial_fs() -> impl Strategy<Value = FsSnapshot> {
    prop::collection::btree_map(
        (0..6u8).prop_map(path),
        prop::collection::vec(any::<u8>(), 0..4),
        0..4,
    )
}

fn apply(fs: &FakeFs, op: &Op) -> Result<OpReceipt, PlatformError> {
    match op {
        Op::Write(p, data) => fs.write_atomic(&path(*p), data),
        Op::Copy(a, b) => fs.copy(&path(*a), &path(*b)),
        Op::Move(a, b) => fs.move_path(&path(*a), &path(*b)),
        Op::Recycle(p) => fs.delete_to_recycle_bin(&path(*p)),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Każda udana operacja jest odwracalna z tokenem, a natychmiastowe cofnięcie
    /// przywraca dokładnie poprzednią migawkę (także zawartość Kosza).
    #[test]
    fn single_op_undo_restores_snapshot(init in initial_fs(), op in op_strategy()) {
        let fs = FakeFs::with_files(init);
        let before = fs.snapshot();
        let bin_before = fs.recycle_bin();
        if let Ok(receipt) = apply(&fs, &op) {
            prop_assert!(receipt.reversible);
            prop_assert!(receipt.op.is_reversible());
            let token = receipt.undo.expect("odwracalna operacja ma token");
            fs.undo(token).unwrap();
            prop_assert_eq!(fs.snapshot(), before);
            prop_assert_eq!(fs.recycle_bin(), bin_before);
            prop_assert_eq!(fs.pending_undo(), 0);
        } else {
            // nieudana operacja nie zmienia stanu
            prop_assert_eq!(fs.snapshot(), before);
        }
    }

    /// Sekwencja operacji cofnięta w odwrotnej kolejności daje stan początkowy.
    #[test]
    fn sequence_undo_lifo_restores_initial(
        init in initial_fs(),
        ops in prop::collection::vec(op_strategy(), 1..12),
    ) {
        let fs = FakeFs::with_files(init);
        let before = fs.snapshot();
        let mut tokens = Vec::new();
        for op in &ops {
            if let Ok(receipt) = apply(&fs, op) {
                tokens.push(receipt.undo.unwrap());
            }
        }
        for token in tokens.into_iter().rev() {
            fs.undo(token).unwrap();
        }
        prop_assert_eq!(fs.snapshot(), before);
        prop_assert!(fs.recycle_bin().is_empty());
    }

    /// Trwałe usunięcie nigdy nie zwraca tokenu i faktycznie usuwa plik.
    #[test]
    fn permanent_delete_is_irreversible(init in initial_fs(), p in 0..6u8) {
        let fs = FakeFs::with_files(init);
        let target = path(p);
        let existed = fs.exists(&target);
        match fs.delete_permanent(&target) {
            Ok(receipt) => {
                prop_assert!(existed);
                prop_assert_eq!(receipt, OpReceipt::irreversible(FsOperation::DeletePermanent));
                prop_assert!(!fs.snapshot().contains_key(&target));
            }
            Err(e) => prop_assert_eq!(e, PlatformError::NotFound(target)),
        }
    }
}

#[test]
fn recycle_bin_keeps_deleted_content_until_undo() {
    let fs = FakeFs::with_files([(PathBuf::from("/a"), b"abc".to_vec())]);
    let receipt = fs.delete_to_recycle_bin(Path::new("/a")).unwrap();
    assert_eq!(
        fs.recycle_bin(),
        vec![(PathBuf::from("/a"), b"abc".to_vec())]
    );
    assert!(!fs.exists(Path::new("/a")));
    fs.undo(receipt.undo.unwrap()).unwrap();
    assert_eq!(fs.read(Path::new("/a")).unwrap(), b"abc");
    assert!(fs.recycle_bin().is_empty());
}
