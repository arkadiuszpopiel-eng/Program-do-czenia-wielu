//! Testy atrapy: kontrakt współdzielony, import przerwany w dowolnym punkcie (property-based,
//! `ACC-F1-transfer-03`), skryptowane błędy i zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use accounts_hub_contract::SecretStore;
use accounts_hub_fake::MemorySecretStore;
use proptest::prelude::*;
use sessions_contract::{NewTurn, Sessions, TreeCursor};
use sessions_fake::FakeSessions;
use transfer_contract::contract_tests::{self, Harness, seed, world};
use transfer_contract::{
    Category, DocumentStore, ExportRequest, ExportScope, ImportMode, ImportOptions, Limits,
    MachineInfo, ModeMap, Selection, Transfer, TransferError, TransferPorts, events,
};
use transfer_fake::{
    FailureBudget, FakeTransfer, FlakySessions, MemoryDocumentStore, SeqIds, VirtualClock,
};

const MACHINE: &str = "0123456789abcdef0123456789abcdef";

struct H {
    transfer: FakeTransfer,
    sessions: Arc<dyn Sessions>,
    stores: BTreeMap<Category, Arc<MemoryDocumentStore>>,
    secrets: Arc<MemorySecretStore>,
    budget: FailureBudget,
}

impl Harness for H {
    fn transfer(&self) -> &dyn Transfer {
        &self.transfer
    }
    fn sessions(&self) -> &dyn Sessions {
        self.sessions.as_ref()
    }
    fn store(&self, category: Category) -> &dyn DocumentStore {
        self.stores[&category].as_ref()
    }
    fn secrets(&self) -> &dyn SecretStore {
        self.secrets.as_ref()
    }
    fn path(&self, name: &str) -> PathBuf {
        PathBuf::from("/atrapa").join(name)
    }
    fn entries(&self, package: &Path) -> Vec<(String, Vec<u8>)> {
        self.transfer.entries(package)
    }
    fn packages_in(&self, dir: &Path) -> Vec<PathBuf> {
        self.transfer.packages_in(dir)
    }
    fn machine_id(&self) -> String {
        MACHINE.to_owned()
    }
}

fn harness() -> H {
    let budget = FailureBudget::unlimited();
    let sessions: Arc<dyn Sessions> = Arc::new(FlakySessions::new(
        Arc::new(FakeSessions::new()),
        budget.clone(),
    ));
    let stores: BTreeMap<Category, Arc<MemoryDocumentStore>> = Category::DOCUMENTS
        .into_iter()
        .map(|c| {
            (
                c,
                Arc::new(MemoryDocumentStore::with_budget(budget.clone())),
            )
        })
        .collect();
    let secrets = Arc::new(MemorySecretStore::new());
    let ports = TransferPorts {
        sessions: Some(sessions.clone()),
        documents: stores
            .iter()
            .map(|(c, s)| (*c, s.clone() as Arc<dyn DocumentStore>))
            .collect(),
        secrets: Some(secrets.clone()),
        machine: MachineInfo {
            id: MACHINE.into(),
            name: "desktop".into(),
            os: "Windows 11".into(),
            hw_class: "standard-amd".into(),
        },
        app_version: semver::Version::new(0, 0, 1),
        workdir_root: None,
        clock: Arc::new(VirtualClock::default()),
        ids: Arc::new(SeqIds::default()),
        limits: Limits::default(),
    };
    H {
        transfer: FakeTransfer::new(ports),
        sessions,
        stores,
        secrets,
        budget,
    }
}

#[test]
fn contract_suite() {
    contract_tests::run_all(harness);
}

#[test]
fn scripted_failure_and_events() {
    let h = harness();
    seed(&h, false);
    h.transfer.fail_next(TransferError::io("dysk pełny"));
    let req = ExportRequest::new(ExportScope::default(), h.path("a.alfa"));
    assert_eq!(
        h.transfer.export(&req).err(),
        Some(TransferError::io("dysk pełny"))
    );
    h.transfer.export(&req).unwrap();
    h.transfer
        .inspect(&h.path("a.alfa"), &ImportOptions::default())
        .unwrap();
    let kinds: Vec<String> = h.transfer.events().into_iter().map(|(k, _)| k).collect();
    assert_eq!(
        kinds,
        vec![events::EXPORT_COMPLETED, events::IMPORT_DRY_RUN]
    );
    assert!(
        h.transfer
            .inspect(&h.path("brak.alfa"), &ImportOptions::default())
            .is_err()
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, failure_persistence: None, ..ProptestConfig::default() })]

    /// `ACC-F1-transfer-03`: import przerwany po `n` zapisach (awaria) → każda sesja ma spójne
    /// drzewo, a po „restarcie” rollback ze snapshotu przywraca dokładnie stan sprzed importu.
    #[test]
    fn interrupted_import_is_consistent_and_rolls_back(n in 0_u64..24, mode in 0_usize..3) {
        let h = harness();
        let seeded = seed(&h, false);
        let pkg = h.path("przerwany.alfa");
        let scope = ExportScope { sessions: Selection::All, memory: Selection::All, ..ExportScope::default() };
        h.transfer.export(&ExportRequest::new(scope, &pkg)).unwrap();
        h.sessions.append_turn(&seeded.s1, Some(seeded.s1_leaf), NewTurn::user("lokalnie")).unwrap();
        h.sessions.delete_session(&seeded.s2).unwrap();
        h.store(Category::ConfigCommon).write("shared.toml", b"[ui]\ntheme = \"light\"\n").unwrap();
        h.store(Category::Memory).remove("global.ndjson").unwrap();
        let before = world(&h);
        let modes = [ImportMode::Add, ImportMode::Merge, ImportMode::Replace];
        let opts = ImportOptions { modes: ModeMap::all(modes[mode]), ..ImportOptions::default() };

        h.budget.allow(n);
        let report = h.transfer.import(&pkg, &opts).unwrap();
        for (_, session) in world(&h).sessions {
            prop_assert!(TreeCursor::empty().check_batch(&session.turns).is_ok());
        }
        h.budget.reset();
        if let Some(snapshot) = report.snapshot {
            let rolled = h.transfer.rollback(&snapshot).unwrap();
            prop_assert!(rolled.failed.is_empty(), "{:?}", rolled);
        }
        prop_assert_eq!(world(&h), before);
    }
}
