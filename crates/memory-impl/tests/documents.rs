//! Pamięć w paczce `.alfa` przez silnik `transfer` (kontrakt) i adapter [`MemoryDocuments`]:
//! round-trip „desktop → laptop” bez utraty danych (F7-06), pamięć sesji prywatnej poza
//! paczką, scalanie (tryb `merge`), wiersz łamiący reguły odrzuca cały dokument.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use memory_contract::{
    Accessor, AgentId, EnginePorts, Layer, MemoryScope, MemoryService, NewMemory, PrivateSessions,
    Provenance, RememberMode, SessionId,
};
use memory_impl::MemoryDocuments;
use transfer_contract::engine::{Engine, ExportSpec};
use transfer_contract::{
    Category, DocumentStore, ExportScope, IdSource, ImportMode, ImportOptions, Limits, MachineInfo,
    MemoryPackage, MemorySource, ModeMap, PackageKind, Selection, SystemClock, TransferPorts,
};

struct Ids(AtomicU64);

impl IdSource for Ids {
    fn new_session_id(&self) -> SessionId {
        SessionId::new(format!("kopia-{}", self.0.fetch_add(1, Ordering::SeqCst)))
    }
}

struct Machine {
    stack: common::Stack,
    ports: TransferPorts,
}

fn machine(name: &str) -> Machine {
    let privacy = Arc::new(PrivateSessions::new());
    privacy.mark_private(SessionId::new("P"));
    let ports = EnginePorts {
        privacy: privacy.clone(),
        ..EnginePorts::deterministic()
    };
    let stack = common::stack_with(ports);
    let memory: Arc<dyn MemoryService> = stack.memory.clone();
    let docs = Arc::new(MemoryDocuments::new(memory, privacy));
    let ports = TransferPorts {
        sessions: None,
        documents: BTreeMap::from([(Category::Memory, docs as Arc<dyn DocumentStore>)]),
        secrets: None,
        machine: MachineInfo {
            id: format!("{name}-id"),
            name: name.into(),
            os: "Windows 11".into(),
            hw_class: "standard".into(),
        },
        app_version: semver::Version::new(0, 7, 0),
        workdir_root: None,
        clock: Arc::new(SystemClock),
        ids: Arc::new(Ids(AtomicU64::new(1))),
        limits: Limits::default(),
    };
    Machine { stack, ports }
}

fn put(m: &dyn MemoryService, scope: MemoryScope, text: &str, subject: Option<&str>) {
    let new = NewMemory {
        subject: subject.map(str::to_owned),
        ..NewMemory::new(scope, Layer::Semantic, text, Provenance::User)
    };
    m.remember_as(&Accessor::Owner, new, RememberMode::Explicit)
        .unwrap();
}

fn export_all(m: &Machine) -> MemorySource {
    let scope = ExportScope {
        memory: Selection::All,
        ..ExportScope::default()
    };
    let spec = ExportSpec {
        kind: PackageKind::Export,
        scope: &scope,
        encryption: None,
        notes: None,
        cancel: None,
    };
    let mut package = MemoryPackage::default();
    let outcome = Engine::new(&m.ports).export(&spec, &mut package).unwrap();
    MemorySource {
        manifest: outcome.manifest,
        package,
        migrations: Vec::new(),
    }
}

fn import(
    m: &Machine,
    source: &mut MemorySource,
    mode: ImportMode,
) -> transfer_contract::ImportReport {
    let options = ImportOptions {
        modes: ModeMap::all(mode),
        ..ImportOptions::default()
    };
    let engine = Engine::new(&m.ports);
    let plan = engine.plan(source, &options).unwrap();
    engine.apply(source, &plan, None).unwrap()
}

fn scope_dump(m: &Machine, scope: &MemoryScope) -> Vec<memory_contract::MemoryEntry> {
    m.stack
        .export_scope(&Accessor::Owner, scope, "test")
        .unwrap()
}

#[test]
fn alfa_round_trip_desktop_to_laptop_without_loss() {
    let desktop = machine("desktop");
    let d: &dyn MemoryService = desktop.stack.memory.as_ref();
    put(
        d,
        MemoryScope::Global,
        "Ulubiony kolor to żółty",
        Some("ulubiony kolor"),
    );
    put(
        d,
        MemoryScope::Global,
        "Ulubiony kolor to zielony",
        Some("ulubiony kolor"),
    );
    put(
        d,
        MemoryScope::Project("dom".into()),
        "Klucz do piwnicy wisi w kuchni",
        None,
    );
    put(
        d,
        MemoryScope::Agent(AgentId::new("beta")),
        "Beta prowadzi kalendarz",
        None,
    );
    put(
        d,
        MemoryScope::Session(SessionId::new("S1")),
        "W tej rozmowie planujemy urlop",
        None,
    );
    put(
        d,
        MemoryScope::Session(SessionId::new("P")),
        "Prywatne: wynik badania krwi",
        None,
    );
    let mut source = export_all(&desktop);
    let paths: Vec<&String> = source.package.entries.keys().collect();
    assert_eq!(
        paths,
        vec![
            "memory/agent/beta.ndjson",
            "memory/global.ndjson",
            "memory/project/dom.ndjson",
            "memory/session/S1.ndjson"
        ],
        "pamięć sesji prywatnej poza paczką"
    );
    assert_eq!(source.manifest.scope.counts.memory_entries, 5);
    let laptop = machine("laptop");
    let report = import(&laptop, &mut source, ImportMode::Add);
    assert_eq!((report.added, report.failed), (4, 0));
    for scope in [
        MemoryScope::Global,
        MemoryScope::Project("dom".into()),
        MemoryScope::Agent(AgentId::new("beta")),
        MemoryScope::Session(SessionId::new("S1")),
    ] {
        assert_eq!(
            scope_dump(&laptop, &scope),
            scope_dump(&desktop, &scope),
            "{scope:?}"
        );
    }
    assert!(scope_dump(&laptop, &MemoryScope::Session(SessionId::new("P"))).is_empty());
    let hits = laptop
        .stack
        .recall_as(
            &Accessor::Owner,
            &memory_contract::RecallRequest::new(vec![MemoryScope::Global], "ulubiony kolor", 3),
        )
        .unwrap();
    assert_eq!(
        hits[0].entry.text, "Ulubiony kolor to zielony",
        "historia wersji przeniesiona"
    );
    put(d, MemoryScope::Global, "Rocznica ślubu 12 czerwca", None);
    let mut again = export_all(&desktop);
    let merged = import(&laptop, &mut again, ImportMode::Merge);
    assert_eq!(merged.failed, 0);
    assert_eq!(scope_dump(&laptop, &MemoryScope::Global).len(), 3);
}

#[test]
fn rule_breaking_line_rejects_whole_document() {
    let desktop = machine("desktop");
    put(
        desktop.stack.memory.as_ref(),
        MemoryScope::Global,
        "Zaufany fakt globalny",
        None,
    );
    let mut source = export_all(&desktop);
    let laptop = machine("laptop");
    let store = laptop.ports.store(Category::Memory).unwrap().clone();
    let good = source.package.entries["memory/global.ndjson"].clone();
    let mut bad: serde_json::Value =
        serde_json::from_slice(good.split(|b| *b == b'\n').next().unwrap()).unwrap();
    bad["id"] = "obca-linia".into();
    bad["provenance"] =
        serde_json::json!({ "kind": "untrusted_content", "source": "https://zla.test" });
    let mut tampered = good.clone();
    tampered.extend_from_slice(serde_json::to_string(&bad).unwrap().as_bytes());
    tampered.push(b'\n');
    assert!(store.write("global.ndjson", &tampered).is_err());
    assert!(
        scope_dump(&laptop, &MemoryScope::Global).is_empty(),
        "nic nie zapisano"
    );
    assert!(store.write("../global.ndjson", &good).is_err());
    assert!(store.read("session/P.ndjson").unwrap().is_none());
    let report = import(&laptop, &mut source, ImportMode::Add);
    assert_eq!(report.added, 1);
    assert!(store.remove("global.ndjson").unwrap());
    assert!(scope_dump(&laptop, &MemoryScope::Global).is_empty());
}
