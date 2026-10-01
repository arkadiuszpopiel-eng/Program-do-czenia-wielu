//! Testy specyficzne dla magazynu F7: szyfrowanie zakresu globalnego, crypto-shredding baz
//! zakresów, zatarcie surowych tabel po `forget` (property), zgodność v0/F7 na jednej bazie sesji,
//! prywatność z katalogu sesji, moduł i zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use core_bus_contract::EventKind;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use lib_sqlstore::database_files;
use lib_sqlstore::rusqlite::Connection;
use memory_contract::{
    Accessor, ForgetTarget, Memory, MemoryScope, MemoryService, NewMemory, PrivacyOracle,
    RememberMode, SessionId, events,
};
use memory_impl::{
    CatalogPrivacy, MemoryModule, MemoryParts, ScopeDbs, SqliteMemory, scope_file_name,
};
use sessions_contract::{KeyVault, NewSession, PrivacyTag, SessionCatalog, SessionDbProvider};

fn raw_contains(conn: &Connection, needle: &str) -> bool {
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    tables.iter().any(|t| {
        let mut stmt = conn.prepare(&format!("SELECT * FROM \"{t}\"")).unwrap();
        let cols = stmt.column_count();
        let mut rows = stmt.query([]).unwrap();
        let mut found = false;
        while let Some(row) = rows.next().unwrap() {
            for i in 0..cols {
                if let Ok(lib_sqlstore::rusqlite::types::ValueRef::Text(b)) = row.get_ref(i) {
                    found |= String::from_utf8_lossy(b).contains(needle);
                }
            }
        }
        found
    })
}

fn file_contains(path: &std::path::Path, needle: &str) -> bool {
    database_files(path).iter().any(|f| {
        std::fs::read(f)
            .unwrap_or_default()
            .windows(needle.len())
            .any(|w| w == needle.as_bytes())
    })
}

#[test]
fn global_scope_encrypted_at_rest_and_crypto_shredded() {
    let s = common::stack();
    let secret = "globalny-sekret-7781";
    let e = s
        .remember_as(
            &Accessor::Owner,
            NewMemory::new(
                MemoryScope::Global,
                memory_contract::Layer::Semantic,
                format!("Hasło do Wi-Fi {secret}"),
                memory_contract::Provenance::User,
            ),
            RememberMode::Explicit,
        )
        .unwrap();
    let path = s
        .dbs
        .root()
        .join(scope_file_name(&MemoryScope::Global).unwrap());
    assert!(path.exists());
    assert!(!file_contains(&path, secret), "jawny tekst w pliku bazy");
    assert!(s.vault.load("alfa/memory/global").unwrap().is_some());
    let report = s
        .forget_as(&Accessor::Owner, &ForgetTarget::Scope(MemoryScope::Global))
        .unwrap();
    assert_eq!(report.shredded, vec![MemoryScope::Global]);
    assert_eq!(report.removed, vec![e.entry_ref()]);
    assert!(s.vault.load("alfa/memory/global").unwrap().is_none());
    assert!(database_files(&path).iter().all(|f| !f.exists()));
    assert!(s.dbs.db(&MemoryScope::Global, false).unwrap().is_none());
    let again = s
        .remember_as(
            &Accessor::Owner,
            NewMemory::new(
                MemoryScope::Global,
                memory_contract::Layer::Semantic,
                "Nowy fakt po zatarciu",
                memory_contract::Provenance::User,
            ),
            RememberMode::Explicit,
        )
        .unwrap();
    assert_eq!(
        s.get_as(&Accessor::Owner, &again.entry_ref()).unwrap().text,
        "Nowy fakt po zatarciu"
    );
}

#[test]
fn v0_and_f7_share_one_session_database() {
    let ports = memory_contract::EnginePorts::system(
        Arc::new(memory_impl::UuidIds),
        Arc::new(memory_contract::PrivateSessions::new()),
    );
    let s = common::stack_with(ports);
    let v0 = SqliteMemory::new(s.provider.clone(), s.index.clone(), s.index.clone()).unwrap();
    let sid = SessionId::new("A");
    let scope = MemoryScope::Session(sid.clone());
    let old = v0
        .remember(
            NewMemory::user_fact(sid.clone(), "Fakt zapisany przez v0"),
            RememberMode::Explicit,
        )
        .unwrap();
    let f7 = s
        .remember_as(
            &Accessor::Owner,
            NewMemory::user_fact(sid.clone(), "Fakt zapisany przez F7"),
            RememberMode::Explicit,
        )
        .unwrap();
    let seen: Vec<String> = Memory::list(s.memory.as_ref(), &scope)
        .unwrap()
        .into_iter()
        .map(|e| e.text)
        .collect();
    assert_eq!(
        seen,
        vec!["Fakt zapisany przez v0", "Fakt zapisany przez F7"]
    );
    assert_eq!(v0.list(&scope).unwrap().len(), 2);
    assert_eq!(
        v0.get(&scope, &f7.id).unwrap().text,
        "Fakt zapisany przez F7"
    );
    let edited = s
        .edit(
            &Accessor::Owner,
            &old.entry_ref(),
            &memory_contract::EntryEdit {
                text: Some("Fakt v0 po edycji".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let v0_hits = v0
        .recall(std::slice::from_ref(&scope), "fakt v0", 5)
        .unwrap();
    assert!(
        v0_hits.iter().all(|h| h.entry.id != old.id),
        "v0 pomija wersje zastąpione"
    );
    assert!(v0_hits.iter().any(|h| h.entry.id == edited.id));
}

#[test]
fn catalog_privacy_treats_private_local_only_and_unknown_as_private() {
    let catalog = Arc::new(sessions_fake::FakeSessions::new());
    let mut ids = Vec::new();
    for privacy in [
        PrivacyTag::Normal,
        PrivacyTag::Private,
        PrivacyTag::LocalOnly,
    ] {
        let meta = catalog
            .create_session(NewSession {
                privacy,
                ..NewSession::default()
            })
            .unwrap();
        ids.push(meta.id);
    }
    let oracle = CatalogPrivacy::new(catalog);
    assert!(!oracle.is_private(&ids[0]));
    assert!(oracle.is_private(&ids[1]) && oracle.is_private(&ids[2]));
    assert!(oracle.is_private(&SessionId::new("nie-ma-takiej")));
}

#[tokio::test]
async fn module_lifecycle_and_events_without_content() {
    let s = common::stack();
    let parts = MemoryParts {
        dbs: s.dbs.clone(),
        indexer: s.index.clone(),
        searcher: s.index.clone(),
        privacy: Arc::new(memory_contract::PrivateSessions::new()),
    };
    let mut module = MemoryModule::new(parts).unwrap();
    assert_eq!(module.health(), HealthStatus::NotStarted);
    assert_eq!(module.manifest().id.as_str(), "memory");
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(bus.clone()));
    module.start(ctx).await.unwrap();
    assert_eq!(module.health(), HealthStatus::Healthy);
    let memory = module.service();
    let e = memory
        .remember_as(
            &Accessor::Owner,
            NewMemory::user_fact(SessionId::new("A"), "poufna treść modułu"),
            RememberMode::Explicit,
        )
        .unwrap();
    memory
        .forget_as(&Accessor::Owner, &ForgetTarget::Entry(e.entry_ref()))
        .unwrap();
    let kind = EventKind::Custom(events::FORGOTTEN.to_owned());
    for _ in 0..200 {
        if !bus.recorded_of_kind(&kind).is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(bus.recorded_of_kind(&kind).len(), 1);
    for ev in bus.recorded() {
        assert!(!ev.payload.to_string().contains("poufna"));
    }
    assert!(e.id.0.starts_with("mem-") && e.id.0.len() > 20, "UUIDv7");
    module.stop().await.unwrap();
}

#[test]
fn forget_session_scrubs_raw_session_tables() {
    let s = common::stack();
    let sid = SessionId::new("A");
    let scope = MemoryScope::Session(sid.clone());
    for i in 0..5 {
        s.remember_as(
            &Accessor::Owner,
            NewMemory::user_fact(sid.clone(), format!("Notatka nr {i} omegasekret")),
            RememberMode::Explicit,
        )
        .unwrap();
    }
    let e = s
        .remember_as(
            &Accessor::Owner,
            NewMemory::user_fact(sid.clone(), "Fakt do awansu omegasekret"),
            RememberMode::Explicit,
        )
        .unwrap();
    s.promote_as(&Accessor::Owner, &e.entry_ref(), MemoryScope::Global)
        .unwrap();
    s.export_scope(&Accessor::Owner, &scope, "session/A.ndjson")
        .unwrap();
    let report = s
        .forget_as(&Accessor::Owner, &ForgetTarget::Session(sid.clone()))
        .unwrap();
    assert_eq!(report.removed.len(), 7);
    assert_eq!(report.stale_exports, vec!["session/A.ndjson".to_owned()]);
    let db = s.provider.session_db(&sid).unwrap();
    db.with(|c| {
        assert!(!raw_contains(c, "omegasekret"));
        Ok::<(), ()>(())
    })
    .unwrap();
    let global = s.dbs.db(&MemoryScope::Global, false).unwrap().unwrap();
    global
        .with(|c| {
            assert!(!raw_contains(c, "omegasekret"));
            Ok::<(), ()>(())
        })
        .unwrap();
    assert_eq!(s.index.doc_count(&sid), 0);
}
