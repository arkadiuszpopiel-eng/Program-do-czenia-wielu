//! Manifest modułu, zdarzenia Audytu na magistrali, magazyn katalogowy, migracja v0 → v1.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use core_bus_contract::EventKind;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use sessions_contract::{BranchId, SessionCatalog, SessionHistory, SessionId};
use transfer_contract::contract_tests::seed;
use transfer_contract::{
    Category, DocumentStore, ExportRequest, ExportScope, ImportOptions, Transfer, events,
    sha256_hex,
};
use transfer_impl::{DirDocumentStore, DirFilter, MODULE_TOML, ZipTransfer};

#[test]
fn manifest_is_valid() {
    let m = core_registry_contract::ModuleManifest::parse_toml(MODULE_TOML).unwrap();
    assert_eq!(m.id.as_str(), "transfer");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
    assert_eq!(m.provides[0].to_string(), "transfer-contract@1");
}

#[tokio::test]
async fn audit_events_are_published() {
    let mut h = common::harness();
    let bus = FakeBus::default();
    let module: &mut ZipTransfer = &mut h.transfer;
    assert_eq!(module.health(), HealthStatus::NotStarted);
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx).await.unwrap();
    assert_eq!(module.health(), HealthStatus::Healthy);
    seed(&h, false);
    let pkg = h.dir.path().join("a.alfa");
    h.transfer
        .export(&ExportRequest::new(ExportScope::default(), &pkg))
        .unwrap();
    h.store(Category::Personas).remove("personas.json").unwrap();
    let report = h.transfer.import(&pkg, &ImportOptions::default()).unwrap();
    h.transfer
        .rollback(report.snapshot.as_ref().unwrap())
        .unwrap();
    tokio::task::yield_now().await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    for kind in [
        events::EXPORT_STARTED,
        events::EXPORT_COMPLETED,
        events::IMPORT_SNAPSHOT_CREATED,
        events::IMPORT_COMPLETED,
        events::ROLLED_BACK,
    ] {
        let n = bus
            .recorded_of_kind(&EventKind::Custom(kind.to_owned()))
            .len();
        assert_eq!(n, 1, "{kind}");
    }
    let completed =
        &bus.recorded_of_kind(&EventKind::Custom(events::EXPORT_COMPLETED.to_owned()))[0];
    assert!(
        !completed
            .payload
            .to_string()
            .contains(&h.dir.path().display().to_string()),
        "bez pełnych ścieżek"
    );
    h.transfer.stop().await.unwrap();
}

trait StoreExt {
    fn store(&self, c: Category) -> &dyn DocumentStore;
}

impl StoreExt for common::H {
    fn store(&self, c: Category) -> &dyn DocumentStore {
        self.stores[&c].as_ref()
    }
}

#[test]
fn dir_store_filters_and_refuses_escapes() {
    let dir = tempfile::tempdir().unwrap();
    let store = DirDocumentStore::new(dir.path().join("config"), DirFilter::flat(&["toml"]));
    store.write("shared.toml", b"a = 1\n").unwrap();
    assert_eq!(
        store.read("shared.toml").unwrap(),
        Some(b"a = 1\n".to_vec())
    );
    for bad in [
        "evil.dll",
        "sub/x.toml",
        "../x.toml",
        "C:/x.toml",
        "CON.toml",
    ] {
        assert!(store.write(bad, b"x").is_err(), "{bad}");
    }
    std::fs::write(dir.path().join("config").join("notatka.txt"), b"x").unwrap();
    assert_eq!(store.list().unwrap(), vec!["shared.toml".to_owned()]);
    assert!(store.remove("shared.toml").unwrap());
    assert!(!store.remove("shared.toml").unwrap());
    assert_eq!(store.read("shared.toml").unwrap(), None);
    let tree = DirDocumentStore::new(dir.path().join("artefakty"), DirFilter::tree());
    tree.write("s1/raport.md", b"# r").unwrap();
    assert_eq!(tree.list().unwrap(), vec!["s1/raport.md".to_owned()]);
    #[cfg(unix)]
    {
        let outside = dir.path().join("poza");
        std::fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, dir.path().join("artefakty").join("link")).unwrap();
        assert!(tree.write("link/evil.md", b"x").is_err());
        assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
    }
}

/// Test migracji v0 → v1 na syntetycznej paczce (manifest v0, nagłówek i tury v0 bez gałęzi).
#[test]
fn v0_package_is_migrated_on_import() {
    let h = common::harness();
    let header =
        br#"{"v":0,"id":"stara-1","title":"Stara sesja","created":"2026-05-01T10:00:00Z"}"#
            .to_vec();
    let turns = concat!(
        "{\"v\":0,\"id\":1,\"parent\":null,\"role\":\"user\",\"text\":\"Pytanie\",\"ts\":\"2026-05-01T10:00:01Z\"}\n",
        "{\"v\":0,\"id\":2,\"parent\":1,\"role\":\"assistant\",\"text\":\"Odpowiedź\",\"ts\":\"2026-05-01T10:00:02Z\"}\n",
        "{\"v\":0,\"id\":3,\"parent\":1,\"role\":\"assistant\",\"text\":\"Ponowiona\",\"ts\":\"2026-05-01T10:00:03Z\"}\n",
    )
    .as_bytes()
    .to_vec();
    let config = b"[ui]\ntheme = \"dark\"\n".to_vec();
    let files = [
        ("config/common/shared.toml", config.clone()),
        ("sessions/stara-1/session.json", header.clone()),
        ("sessions/stara-1/turns.ndjson", turns.clone()),
    ];
    let manifest = serde_json::json!({
        "format": 0,
        "app": "0.0.1",
        "created": "2026-05-01T10:00:00Z",
        "machine": common::MACHINE,
        "files": files.iter().map(|(n, b)| serde_json::json!({ "name": n, "sha256": sha256_hex(b), "size": b.len() })).collect::<Vec<_>>(),
    });
    let pkg = h.dir.path().join("v0.alfa");
    common::craft(
        &pkg,
        Some(manifest),
        &files
            .iter()
            .map(|(n, b)| (*n, b.clone()))
            .collect::<Vec<_>>(),
    );
    let inspection = h.transfer.inspect(&pkg, &ImportOptions::default()).unwrap();
    let steps: Vec<(String, String, u64)> = inspection
        .report
        .migrations
        .iter()
        .map(|m| (m.entity.clone(), m.from.clone(), m.count))
        .collect();
    assert_eq!(
        steps,
        vec![
            ("manifest".into(), "0".into(), 1),
            ("session".into(), "0".into(), 1),
            ("turn".into(), "0".into(), 3)
        ]
    );
    let report = h.transfer.import(&pkg, &ImportOptions::default()).unwrap();
    assert_eq!((report.added, report.failed), (2, 0));
    let id = SessionId::new("stara-1");
    assert_eq!(h.sessions.session(&id).unwrap().title, "Stara sesja");
    let branches: Vec<BranchId> = h
        .sessions
        .all_turns(&id)
        .unwrap()
        .iter()
        .map(|t| t.branch)
        .collect();
    assert_eq!(branches, vec![BranchId(1), BranchId(1), BranchId(2)]);
    assert_eq!(
        h.stores[&Category::ConfigCommon]
            .read("shared.toml")
            .unwrap(),
        Some(config)
    );
}
