//! Księga w pliku: propozycje (oczekujące i rozstrzygnięte) przeżywają restart modułu; oczekującą
//! można zatwierdzić po restarcie.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use core_bus_fake::FakeBus;
use core_registry_contract::{Module, ModuleContext};
use marshal_contract::{Approver, Marshal, ProposalStatus};
use marshal_impl::{FileMarshalStore, MarshalModule, NoTranslator};
use serde_json::json;

async fn started(store: Arc<FileMarshalStore>) -> MarshalModule {
    let mut module = MarshalModule::new(Arc::new(NoTranslator), store)
        .unwrap()
        .with_start_ms(1_790_841_600_000);
    let ctx = ModuleContext::new(module.manifest().id.clone(), Arc::new(FakeBus::default()));
    module.start(ctx).await.unwrap();
    module
}

#[tokio::test]
async fn proposals_survive_module_restart() {
    let dir = std::env::temp_dir().join(format!("alfa-marshal-trwalosc-{}", std::process::id()));
    let store = Arc::new(FileMarshalStore::new(dir.join("marshal.json")));
    let mut first = started(store.clone()).await;
    assert!(first.proposals().is_empty());
    let rule = |id: &str| json!({"id": id, "then": [{"effect": "deny_bridges"}]});
    let a = first.propose_drafts("bez mostów", vec![rule("a")]);
    let b = first.propose_drafts("czeka", vec![rule("b")]);
    first.approve(a.id, Approver::UserInterface).unwrap();
    let before = first.proposals();
    first.stop().await.unwrap();
    assert!(
        first.proposals().is_empty(),
        "zatrzymany moduł nie ma stanu"
    );

    let second = started(store).await;
    assert_eq!(second.proposals(), before);
    assert_eq!(second.proposals()[0].status, ProposalStatus::Pending);
    second.approve(b.id, Approver::UserInterface).unwrap();
    assert_eq!(second.rules().len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
