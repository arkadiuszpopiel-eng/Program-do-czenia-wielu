//! Kontrakt współdzielony na prawdziwych bazach SQLCipher (indeks: atrapa `FakeSearch`),
//! szyfrowanie wpisów, kaskada w surowej tabeli, moduł i zdarzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use core_bus_contract::EventKind;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use memory_contract::contract_tests;
use memory_contract::{Memory, MemoryScope, NewMemory, RememberMode, SessionId, events};
use memory_impl::SqliteMemory;
use search_fake::FakeSearch;
use sessions_contract::SessionDbProvider;
use sessions_fake::TempDbProvider;

struct Harness {
    provider: Arc<TempDbProvider>,
    index: Arc<FakeSearch>,
    memory: SqliteMemory,
}

impl Deref for Harness {
    type Target = SqliteMemory;

    fn deref(&self) -> &SqliteMemory {
        &self.memory
    }
}

fn harness() -> Harness {
    let provider = Arc::new(TempDbProvider::new().unwrap());
    let index = Arc::new(FakeSearch::new());
    let memory = SqliteMemory::new(provider.clone(), index.clone(), index.clone()).unwrap();
    Harness {
        provider,
        index,
        memory,
    }
}

#[test]
fn contract_suite() {
    contract_tests::run_all(harness);
}

#[test]
fn entries_live_encrypted_in_session_db_and_forget_removes_row() {
    let h = harness();
    let secret = "pamięć-szpieg-5521";
    let s = SessionId::new("A");
    let scope = MemoryScope::Session(s.clone());
    let e = h
        .remember(
            NewMemory::user_fact(s.clone(), secret),
            RememberMode::Explicit,
        )
        .unwrap();
    assert_eq!(h.index.doc_count(&s), 1);
    let db = h.provider.session_db(&s).unwrap();
    let rows = |db: &lib_sqlstore::Db| {
        db.with(|c| {
            c.query_row("SELECT count(*) FROM memory_entries", [], |r| {
                r.get::<_, i64>(0)
            })
        })
        .unwrap()
    };
    assert_eq!(rows(&db), 1);
    for file in lib_sqlstore::database_files(db.path()) {
        let bytes = std::fs::read(&file).unwrap_or_default();
        assert!(!bytes.windows(secret.len()).any(|w| w == secret.as_bytes()));
    }
    h.forget(&scope, &e.id).unwrap();
    assert_eq!(rows(&db), 0);
    assert_eq!(h.index.doc_count(&s), 0);
}

#[tokio::test]
async fn module_lifecycle_and_events_without_content() {
    let mut h = harness();
    assert_eq!(h.memory.health(), HealthStatus::NotStarted);
    assert_eq!(h.memory.manifest().id.as_str(), "memory");
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(h.memory.manifest().id.clone(), Arc::new(bus.clone()));
    h.memory.start(ctx).await.unwrap();
    let s = SessionId::new("A");
    let e = h
        .remember(
            NewMemory::user_fact(s.clone(), "poufny fakt"),
            RememberMode::Explicit,
        )
        .unwrap();
    h.forget(&MemoryScope::Session(s), &e.id).unwrap();
    let kind = EventKind::Custom(events::FORGOTTEN.to_owned());
    for _ in 0..200 {
        if !bus.recorded_of_kind(&kind).is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let remembered = bus.recorded_of_kind(&EventKind::Custom(events::REMEMBERED.to_owned()));
    assert_eq!(remembered.len(), 1);
    assert!(!remembered[0].payload.to_string().contains("poufny"));
    assert_eq!(bus.recorded_of_kind(&kind).len(), 1);
    h.memory.stop().await.unwrap();
}
