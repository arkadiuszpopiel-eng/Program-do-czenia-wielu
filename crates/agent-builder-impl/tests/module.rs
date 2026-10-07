//! Moduł Kreatora: kontrakt współdzielony (ścieżka szczęśliwa, ≥ 30 ataków = 0 sukcesów) na
//! `personas-fake`, zapis trafia do `personas` i biblioteki, trwałość manifestów, sufit z sesji,
//! zdarzenia, cykl życia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use agent_builder_contract::contract_tests as ct;
use agent_builder_contract::{AgentBuilder, BuildError, BuilderApproval, BuilderApprovalOrigin};
use agent_builder_impl::{
    AgentBuilderModule, CeilingSource, DirManifestStore, FixedCeiling, ManifestStore,
    MemManifestStore, module_manifest,
};
use core_bus_fake::FakeBus;
use core_registry_contract::{HealthStatus, Module, ModuleContext};
use personas_contract::{PersonaId, Personas};
use personas_fake::FakePersonas;
use risk_classifier_contract::AutonomyLevel;

async fn personas() -> Arc<FakePersonas> {
    Arc::new(FakePersonas::new())
}

async fn module_with(
    store: Arc<dyn ManifestStore>,
    ceiling: AutonomyLevel,
) -> (AgentBuilderModule, Arc<FakePersonas>, FakeBus) {
    let p = personas().await;
    let mut m = AgentBuilderModule::new(
        p.clone(),
        ct::sample_tools(),
        store,
        Arc::new(FixedCeiling(ceiling)),
    )
    .unwrap();
    let bus = FakeBus::default();
    m.start(ModuleContext::new(
        m.manifest().id.clone(),
        Arc::new(bus.clone()),
    ))
    .await
    .unwrap();
    (m, p, bus)
}

async fn module() -> (AgentBuilderModule, Arc<FakePersonas>, FakeBus) {
    module_with(Arc::new(MemManifestStore::default()), AutonomyLevel::L3).await
}

#[tokio::test]
async fn contract_suite_and_personas_updated() {
    let (m, p, bus) = module().await;
    ct::happy_path(&m).await;
    let ola = p
        .personas()
        .into_iter()
        .find(|x| x.id == PersonaId::new("ola"))
        .unwrap();
    assert_eq!(ola.forms.dative, "Oli");
    assert!(p.roles().iter().any(|r| !r.builtin));
    let kinds: Vec<String> = bus
        .recorded()
        .iter()
        .map(|e| e.kind.as_str().to_owned())
        .collect();
    assert!(
        kinds.contains(&"agent_builder.dry_run".to_owned())
            && kinds.contains(&"agent_builder.saved".to_owned())
    );
}

#[tokio::test]
async fn attacks_never_succeed_on_impl() {
    let (m, p, _) = module().await;
    let before = p.personas().len();
    let (tries, wins) = ct::attacks(&m).await;
    eprintln!("Kreator (impl): {tries} prób ataku, {wins} sukcesów");
    assert_eq!(wins, 0);
    assert_eq!(p.personas().len(), before, "żadna persona nie dodana");
}

struct Session(std::sync::Mutex<AutonomyLevel>);

impl CeilingSource for Session {
    fn ceiling(&self) -> AutonomyLevel {
        *self.0.lock().unwrap()
    }
}

#[tokio::test]
async fn ceiling_follows_session_level() {
    let p = personas().await;
    let s = Arc::new(Session(std::sync::Mutex::new(AutonomyLevel::L3)));
    let m = AgentBuilderModule::new(
        p,
        ct::sample_tools(),
        Arc::new(MemManifestStore::default()),
        s.clone(),
    )
    .unwrap();
    let d = m.propose(ct::DESCRIPTION).draft;
    assert_eq!(
        m.build(&d).unwrap().manifest.limits.autonomy,
        AutonomyLevel::L3
    );
    *s.0.lock().unwrap() = AutonomyLevel::L1;
    assert_eq!(
        m.build(&d).unwrap().manifest.limits.autonomy,
        AutonomyLevel::L1,
        "obniżenie sesji obniża sufit"
    );
    *s.0.lock().unwrap() = AutonomyLevel::L4;
    assert_eq!(m.policy().ceiling, AutonomyLevel::L3);
}

#[tokio::test]
async fn manifests_persist_and_lifecycle() {
    assert_eq!(module_manifest().unwrap().id.as_str(), "agent-builder");
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(DirManifestStore::open(dir.path()).unwrap());
    let (m, _, _) = module_with(store.clone(), AutonomyLevel::L3).await;
    ct::happy_path(&m).await;
    assert_eq!(store.load().unwrap().len(), 1);
    let again = AgentBuilderModule::new(
        personas().await,
        ct::sample_tools(),
        store,
        Arc::new(FixedCeiling(AutonomyLevel::L2)),
    )
    .unwrap();
    assert_eq!(again.library().len(), 1);
    assert_eq!(again.health(), HealthStatus::NotStarted);
    let d = again.propose(ct::DESCRIPTION).draft;
    let built = again.build(&d).unwrap();
    let ui = BuilderApproval {
        origin: BuilderApprovalOrigin::Ui,
        reviewed_hash: built.hash.clone(),
    };
    assert!(
        matches!(
            again.save(&built.manifest, ui).await,
            Err(BuildError::Store(_))
        ),
        "bez startu — brak zapisu"
    );
    assert!(matches!(
        again.dry_run(&built.manifest, &ct::scenario()).await,
        Err(BuildError::Store(_))
    ));
}

#[tokio::test]
async fn personas_failure_saves_nothing() {
    let (m, p, _) = module().await;
    let d = m.propose(ct::DESCRIPTION).draft;
    let built = m.build(&d).unwrap();
    assert!(
        m.dry_run(&built.manifest, &ct::scenario())
            .await
            .unwrap()
            .passed
    );
    p.fail_next(personas_contract::PersonasError::NotStarted);
    let ui = BuilderApproval {
        origin: BuilderApprovalOrigin::Ui,
        reviewed_hash: built.hash.clone(),
    };
    assert!(matches!(
        m.save(&built.manifest, ui).await,
        Err(BuildError::Store(_))
    ));
    assert!(m.library().is_empty());
}
