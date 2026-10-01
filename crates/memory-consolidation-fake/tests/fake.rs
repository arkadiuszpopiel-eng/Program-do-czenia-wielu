//! Atrapy portów: kolejka odpowiedzi, uzupełnianie zużycia, werdykt budżetu, stan maszyny.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use memory_consolidation_contract::{
    BackgroundBudget, BudgetVerdict, ConsolidationBatch, ConsolidationError, Consolidator,
    ConsolidatorOutput, HostConditions,
};
use memory_consolidation_fake::{FakeHost, FixedBudget, ScriptedConsolidator};
use memory_contract::{MemoryScope, SessionId};

#[tokio::test]
async fn ports_are_scripted_and_recorded() {
    let batch = ConsolidationBatch {
        scope: MemoryScope::Session(SessionId::new("A")),
        private: false,
        episodes: vec![],
        known_facts: vec![],
    };
    let c = ScriptedConsolidator::cloud(700);
    assert!(!c.model().local && c.estimate_micro_usd(&batch) == Some(700));
    c.push(Err(ConsolidationError::new("awaria")));
    assert!(c.consolidate(&batch).await.is_err());
    let out = c.consolidate(&batch).await.unwrap();
    assert_eq!(out.facts, ConsolidatorOutput::default().facts);
    assert_eq!(out.usage.unwrap().cost_micro_usd, Some(700));
    assert_eq!(c.batches().len(), 2);
    let b = FixedBudget::deny();
    assert!(matches!(
        b.check(&c.model(), Some(1)).await,
        BudgetVerdict::Deny { .. }
    ));
    b.set(BudgetVerdict::Allow);
    assert_eq!(b.check(&c.model(), None).await, BudgetVerdict::Allow);
    assert_eq!(b.checks(), vec![Some(1), None]);
    let h = FakeHost::idle_night();
    h.update(|s| s.on_battery = true);
    assert!(h.state().on_battery && !h.state().fullscreen);
    assert!(ScriptedConsolidator::local().model().local);
}
