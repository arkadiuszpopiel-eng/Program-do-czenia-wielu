//! Strażniczka pamięci na silniku atrapy pamięci: reguły deterministyczne i cofnięcie przebiegu,
//! ekstrakcja modelem (tryb „ask”, umiejętności, walidacja źródeł).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use memory_consolidation_contract::{
    ConsolidationConfig, ConsolidatorOutput, ProposedFact, ProposedSkill, ProposedSummary, Trigger,
    undo_run,
};
use memory_consolidation_fake::ScriptedConsolidator;
use memory_contract::{
    Accessor, EntryState, Layer, NewMemory, Provenance, RememberMode, entry_state,
};

mod common;

use common::{all, guardian, put, sess, world};

#[tokio::test]
async fn deterministic_rules_then_undo_run() {
    let w = world(ScriptedConsolidator::local());
    let m = w.memory.as_ref();
    let a = sess("A");
    let d1 = put(m, a.clone(), Layer::Semantic, "Telefon Anny: 600 100 200");
    let d2 = put(m, a.clone(), Layer::Semantic, "Telefon Anny 600 100 200");
    let old = put(m, a.clone(), Layer::Semantic, "Ulubiony kolor to żółty");
    let new = put(m, a.clone(), Layer::Semantic, "Ulubiony kolor to zielony");
    let ttl = m
        .remember_as(
            &Accessor::Owner,
            NewMemory {
                ttl_secs: Some(10),
                ..NewMemory::new(
                    a.clone(),
                    Layer::Semantic,
                    "Kod tymczasowy 4711",
                    Provenance::User,
                )
            },
            RememberMode::Explicit,
        )
        .unwrap();
    w.clock.advance(3600);
    let g = guardian(&w, ConsolidationConfig::default(), w.host.clone());
    let report = g.run(Trigger::Scheduled).await;
    assert_eq!(report.skipped, None);
    let run = &report.scopes[0];
    assert_eq!(
        (run.merged, run.resolved, run.expired),
        (1, 1, 1),
        "{report:?}"
    );
    let entries = all(m, &a);
    let now = memory_contract::MemoryClock::now(w.clock.as_ref());
    let state = |id: &memory_contract::MemoryId| {
        entries
            .iter()
            .find(|e| &e.id == id)
            .map(|e| entry_state(e, now))
    };
    assert_eq!(
        [state(&d1.id), state(&d2.id)]
            .iter()
            .filter(|s| **s == Some(EntryState::Superseded))
            .count(),
        1
    );
    assert_eq!(state(&old.id), Some(EntryState::Superseded));
    assert_eq!(state(&new.id), Some(EntryState::Active));
    assert_eq!(state(&ttl.id), None, "wygasły usunięty");
    let (undone, skipped) = undo_run(m, &report.run).unwrap();
    assert_eq!((undone, skipped), (2, 1), "wygaszenie nieodwracalne");
    let entries = all(m, &a);
    assert!(entries.iter().all(|e| e.superseded.is_none()));
    assert!(
        w.events
            .events()
            .iter()
            .any(|(k, _)| k == "memory.consolidation.finished")
    );
}

#[tokio::test]
async fn model_extraction_ask_mode_skills_and_validation() {
    let w = world(ScriptedConsolidator::local());
    let m = w.memory.as_ref();
    let a = sess("A");
    let e1 = put(
        m,
        a.clone(),
        Layer::Episodic,
        "Rozmowa: użytkownik pije kawę bez cukru",
    );
    let e2 = put(
        m,
        a.clone(),
        Layer::Episodic,
        "Rozmowa: robimy kopię zapasową w piątki",
    );
    w.model.push(Ok(ConsolidatorOutput {
        facts: vec![
            ProposedFact {
                text: "Użytkownik pije kawę bez cukru".into(),
                subject: None,
                entities: vec![],
                confidence: 0.9,
                sources: vec![e1.id.clone()],
            },
            ProposedFact {
                text: "Zmyślony fakt bez źródła".into(),
                subject: None,
                entities: vec![],
                confidence: 0.9,
                sources: vec![memory_contract::MemoryId("obcy".into())],
            },
        ],
        summaries: vec![ProposedSummary {
            text: "Streszczenie: kawa i kopie".into(),
            sources: vec![e1.id.clone(), e2.id.clone()],
        }],
        skills: vec![ProposedSkill {
            title: "Kopia zapasowa".into(),
            text: "w piątki eksport .alfa".into(),
            sources: vec![e2.id.clone()],
        }],
        usage: None,
    }));
    let g = guardian(&w, ConsolidationConfig::default(), w.host.clone());
    let report = g.run(Trigger::Manual).await;
    assert_eq!(
        (
            report.llm_calls,
            report.scopes[0].created,
            report.scopes[0].rejected
        ),
        (1, 3, 1)
    );
    let entries = all(m, &a);
    let find = |t: &str| {
        entries
            .iter()
            .find(|e| e.text.starts_with(t))
            .cloned()
            .unwrap()
    };
    let fact = find("Użytkownik pije");
    assert!(
        !fact.approved && fact.origin.derived_from.len() == 1,
        "auto_extract = ask → oczekujący"
    );
    assert!(matches!(fact.provenance, Provenance::Agent { .. }));
    assert!(find("Streszczenie").approved);
    let skill = find("Kopia zapasowa");
    assert!(skill.layer == Layer::Procedural && !skill.approved);
    assert!(
        entries
            .iter()
            .filter(|e| e.layer == Layer::Episodic && e.origin.derivation.is_none())
            .all(|e| e.consolidated_at.is_some())
    );
    assert_eq!(w.budget.records().len(), 1);
    let again = g.run(Trigger::Manual).await;
    assert_eq!(again.llm_calls, 0, "epizody już przetworzone");
    assert_eq!(w.model.batches().len(), 1);
}
