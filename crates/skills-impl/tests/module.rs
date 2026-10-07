//! Moduł `skills`: kontrakt współdzielony, cykl życia i manifest, trwałość (zapis atomowy,
//! błąd zapisu = brak zmiany), zdarzenia na magistrali, uruchamianie przez `agent-runtime`
//! (koperta ≤ roli), dokument `.alfa` (import = propozycje, nigdy instalacja).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use agent_runtime_contract::{AgentRuntime, RunOptions};
use agent_runtime_fake::FakeAgentRuntime;
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use serde_json::json;
use skills_contract::contract_tests as ct;
use skills_contract::{
    ApprovalOrigin, BUNDLE_DOCUMENT, OwnerApproval, SkillError, SkillRecord, SkillSource,
    SkillState, Skills,
};
use skills_impl::{
    DirSkillStore, MemSkillStore, SkillRunner, SkillStore, SkillsDocuments, SkillsModule,
    module_manifest,
};
use transfer_contract::DocumentStore;

async fn started(store: Arc<dyn SkillStore>) -> (SkillsModule, FakeBus) {
    let mut m = SkillsModule::new(ct::sample_catalog(), store).unwrap();
    let bus = FakeBus::default();
    let ctx = ModuleContext::new(m.manifest().id.clone(), Arc::new(bus.clone()));
    m.start(ctx).await.unwrap();
    (m, bus)
}

async fn module() -> SkillsModule {
    started(Arc::new(MemSkillStore::default())).await.0
}

async fn install(m: &dyn Skills, version: &str) -> SkillRecord {
    let r = m
        .propose(ct::sample_skill(version), SkillSource::User)
        .await
        .unwrap();
    let a = OwnerApproval {
        origin: ApprovalOrigin::Ui,
        reviewed_hash: r.hash.clone(),
    };
    m.approve(&r.skill.id, &r.skill.version, a).await.unwrap()
}

#[tokio::test]
async fn contract_suite() {
    ct::lifecycle(&module().await).await;
    ct::quarantine(&module().await).await;
    ct::export_import(&module().await, &module().await).await;
}

#[tokio::test]
async fn lifecycle_manifest_and_not_started() {
    assert_eq!(module_manifest().unwrap().id.as_str(), "skills");
    let mut m =
        SkillsModule::new(ct::sample_catalog(), Arc::new(MemSkillStore::default())).unwrap();
    assert_eq!(m.health(), HealthStatus::NotStarted);
    assert!(matches!(
        m.propose(ct::sample_skill("1.0.0"), SkillSource::User)
            .await,
        Err(SkillError::Store(_))
    ));
    let ctx = ModuleContext::new(m.manifest().id.clone(), Arc::new(FakeBus::default()));
    m.start(ctx.clone()).await.unwrap();
    assert!(m.start(ctx).await.is_err());
    assert_eq!(m.health(), HealthStatus::Healthy);
    m.stop().await.unwrap();
    assert!(m.stop().await.is_err());
}

#[tokio::test]
async fn persists_and_publishes() {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(DirSkillStore::open(dir.path()).unwrap());
    let (m, bus) = started(store.clone()).await;
    let inst = install(&m, "1.0.0").await;
    let kinds: Vec<String> = bus
        .recorded()
        .iter()
        .map(|e| e.kind.as_str().to_owned())
        .collect();
    assert_eq!(kinds, vec!["skills.proposed", "skills.installed"]);
    let (again, _) = started(Arc::new(DirSkillStore::open(dir.path()).unwrap())).await;
    assert_eq!(again.installed(&inst.skill.id).unwrap().hash, inst.hash);
    assert!(store.path().ends_with("skills.json"));
}

struct Broken;

impl SkillStore for Broken {
    fn load(&self) -> Result<Vec<SkillRecord>, String> {
        Ok(Vec::new())
    }
    fn save(&self, _r: &[SkillRecord]) -> Result<(), String> {
        Err("dysk pełny".into())
    }
}

#[tokio::test]
async fn failed_save_changes_nothing() {
    let (m, bus) = started(Arc::new(Broken)).await;
    assert!(matches!(
        m.propose(ct::sample_skill("1.0.0"), SkillSource::User)
            .await,
        Err(SkillError::Store(_))
    ));
    assert!(m.list().is_empty());
    assert!(bus.recorded().is_empty());
}

#[tokio::test(start_paused = true)]
async fn runner_starts_runtime_with_attenuated_grant() {
    let m: Arc<dyn Skills> = Arc::new(module().await);
    let inst = install(m.as_ref(), "1.0.0").await;
    let rt = Arc::new(FakeAgentRuntime::new());
    let runner = SkillRunner::new(m.clone(), rt.clone());
    let parent = rt.start(ct::caller("operator")).await.unwrap();
    let run = runner
        .run(
            &inst.skill.id,
            &json!({"folder": "C:/Pobrane"}),
            &ct::caller("operator"),
            &RunOptions::default(),
            Some(parent.clone()),
        )
        .await
        .unwrap();
    let opts = rt.options(&run).unwrap();
    assert_eq!(opts.parent, Some(parent.clone()));
    let grant = opts.grant.unwrap();
    assert_eq!(
        grant.tools.iter().cloned().collect::<Vec<_>>(),
        vec!["fs_list".to_owned(), "fs_move".to_owned()]
    );
    assert!(opts.label.unwrap().contains("Porządki w Pobranych"));
    assert_eq!(rt.children(&parent).unwrap(), vec![run]);
    let denied = runner
        .run(
            &inst.skill.id,
            &json!({"folder": "x"}),
            &ct::caller("critic"),
            &RunOptions::default(),
            None,
        )
        .await;
    assert_eq!(denied, Err(SkillError::ExceedsRole("fs_move".into())));
    let roles: Vec<_> = ct::caller("operator").roles;
    let hits = runner.suggest("posortuj Pobrane", &roles, None, 3).await;
    assert_eq!(hits[0].id, inst.skill.id);
}

#[tokio::test]
async fn alfa_document_round_trip_never_installs() {
    let a = Arc::new(module().await);
    let docs_a = SkillsDocuments::new(a.clone());
    assert!(docs_a.list().unwrap().is_empty());
    let inst = install(a.as_ref(), "1.0.0").await;
    assert_eq!(docs_a.list().unwrap(), vec![BUNDLE_DOCUMENT.to_owned()]);
    let bytes = docs_a.read(BUNDLE_DOCUMENT).unwrap().unwrap();
    assert!(docs_a.read("inny.json").unwrap().is_none());
    let b = Arc::new(module().await);
    let docs_b = SkillsDocuments::new(b.clone());
    docs_b.write(BUNDLE_DOCUMENT, &bytes).unwrap();
    let rec = b
        .list()
        .into_iter()
        .find(|r| r.skill.id == inst.skill.id)
        .unwrap();
    assert_eq!(
        rec.state,
        SkillState::Proposed,
        "import własnej paczki = propozycja"
    );
    assert!(b.installed(&inst.skill.id).is_none());
    let mut tampered = String::from_utf8(bytes).unwrap();
    tampered = tampered.replacen("Porządki", "Porzadki", 1);
    assert!(docs_b.write(BUNDLE_DOCUMENT, tampered.as_bytes()).is_err());
    assert!(docs_b.write("obcy.json", b"{}").is_err());
    assert!(!docs_b.remove(BUNDLE_DOCUMENT).unwrap());
}
