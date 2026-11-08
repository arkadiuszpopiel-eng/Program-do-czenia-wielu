//! Reguły deterministyczne: temat z wzorców, duplikaty (także property), sprzeczności, retencja,
//! walidacja propozycji modelu.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::{DateTime, Duration, Utc};
use memory_consolidation_contract::rules::{
    contradictions, duplicates, jaccard, proposals_to_ops, retention, subject_of,
};
use memory_consolidation_contract::{
    AutoExtract, ConsolidationBatch, ConsolidationConfig, ConsolidatorOutput, EpisodeView,
    ProposedFact,
};
use memory_contract::{
    AgentId, ChangeOp, Layer, MemoryEntry, MemoryId, MemoryScope, NewMemory, Provenance, SessionId,
};
use proptest::prelude::*;

fn entry(id: &str, text: &str, secs: i64, provenance: Provenance) -> MemoryEntry {
    let new = NewMemory::new(
        MemoryScope::Session(SessionId::new("A")),
        Layer::Semantic,
        text,
        provenance,
    );
    MemoryEntry::from_new(
        MemoryId(id.into()),
        new,
        DateTime::<Utc>::default() + Duration::seconds(secs),
        true,
    )
}

fn now() -> DateTime<Utc> {
    DateTime::<Utc>::default() + Duration::days(1)
}

#[test]
fn subject_patterns() {
    let s = |t: &str| subject_of(&entry("x", t, 0, Provenance::User));
    assert_eq!(s("Ulubiony kolor to żółty."), Some("ulubiony kolor".into()));
    assert_eq!(s("Numer buta: 43"), Some("numer buta".into()));
    assert_eq!(
        s("Rata kredytu wynosi 1200 zł"),
        Some("rata kredytu".into())
    );
    assert_eq!(s("Lubię spacery"), None);
    assert_eq!(
        s("To jest bardzo długi temat z wieloma słowami w środku to x"),
        None
    );
    let mut e = entry("x", "Cokolwiek", 0, Provenance::User);
    e.subject = Some("Ulubiony  KOLOR".into());
    assert_eq!(subject_of(&e), Some("ulubiony kolor".into()));
    assert!((jaccard(&["a".into()], &["a".into()]) - 1.0).abs() < f32::EPSILON);
    assert_eq!(jaccard(&[], &[]), 0.0);
}

#[test]
fn contradictions_respect_trust_and_flag_once() {
    let user = entry("u", "Ulubiony kolor to żółty", 0, Provenance::User);
    let agent = entry(
        "a",
        "Ulubiony kolor to czerwony",
        10,
        Provenance::Agent {
            agent: AgentId::new("beta"),
        },
    );
    let ops = contradictions(
        &[user.clone(), agent.clone()],
        &Default::default(),
        &[],
        now(),
    );
    assert!(matches!(&ops[..], [ChangeOp::FlagConflict { .. }]));
    let newer_user = entry("n", "Ulubiony kolor to zielony", 20, Provenance::User);
    let ops = contradictions(&[user, agent, newer_user], &Default::default(), &[], now());
    assert_eq!(
        ops.iter()
            .filter(|o| matches!(o, ChangeOp::Resolve { .. }))
            .count(),
        2
    );
}

#[test]
fn retention_expires_ttl_and_old_processed_episodes_but_not_pinned() {
    let cfg = ConsolidationConfig {
        episodic_retention_days: Some(1),
        ..Default::default()
    };
    let mut ttl = entry("t", "ttl", 0, Provenance::User);
    ttl.ttl_secs = Some(5);
    let mut old_ep = entry("e", "epizod", 0, Provenance::User);
    old_ep.layer = Layer::Episodic;
    old_ep.consolidated_at = Some(DateTime::<Utc>::default());
    let mut pinned = old_ep.clone();
    pinned.id = MemoryId("p".into());
    pinned.pinned = true;
    let mut fresh = old_ep.clone();
    fresh.id = MemoryId("f".into());
    fresh.consolidated_at = None;
    let later = DateTime::<Utc>::default() + Duration::days(3);
    let ids: Vec<MemoryId> = retention(&[ttl, old_ep, pinned, fresh], later, &cfg)
        .into_iter()
        .filter_map(|op| match op {
            ChangeOp::Expire { id, .. } => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(ids, vec![MemoryId("t".into()), MemoryId("e".into())]);
}

#[test]
fn proposals_are_validated_and_capped() {
    let batch = ConsolidationBatch {
        scope: MemoryScope::Session(SessionId::new("A")),
        private: false,
        episodes: vec![EpisodeView {
            id: MemoryId("ep".into()),
            text: "x".into(),
            created_at: DateTime::default(),
        }],
        known_facts: vec![],
    };
    let fact = |text: &str, src: &str| ProposedFact {
        text: text.into(),
        subject: None,
        entities: vec![],
        confidence: f32::NAN,
        sources: vec![MemoryId(src.into())],
    };
    let mut facts: Vec<ProposedFact> = (0..30)
        .map(|i| fact(&format!("Fakt numer {i}"), "ep"))
        .collect();
    facts.push(fact("Obce źródło", "obce"));
    let existing = vec![entry("old", "Fakt numer 0", 0, Provenance::User)];
    let out = ConsolidatorOutput {
        facts,
        ..Default::default()
    };
    let cfg = ConsolidationConfig {
        auto_extract: AutoExtract::On,
        ..Default::default()
    };
    let (ops, rejected) = proposals_to_ops(&batch, &out, &existing, &cfg);
    let creates: Vec<&ChangeOp> = ops
        .iter()
        .filter(|o| matches!(o, ChangeOp::Create { .. }))
        .collect();
    assert_eq!(creates.len(), 19, "limit 20, jeden duplikat znanego faktu");
    assert_eq!(rejected, 12);
    assert!(
        matches!(ops.last(), Some(ChangeOp::MarkConsolidated { ids }) if ids == &vec![MemoryId("ep".into())])
    );
    for op in creates {
        if let ChangeOp::Create {
            entry, approved, ..
        } = op
        {
            assert!(*approved && entry.confidence == 0.5 && entry.provenance.is_trusted());
            assert!(entry.origin.derived_from.iter().all(|r| r.id.0 == "ep"));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, failure_persistence: None, ..ProptestConfig::default() })]

    /// Scalenie: wiodący nie jest duplikatem, grupy rozłączne, wiodący ma najwyższe zaufanie w grupie.
    #[test]
    fn merges_are_disjoint_and_keep_most_trusted(texts in proptest::collection::vec(prop_oneof!["kawa bez cukru", "Kawa bez cukru!", "herbata", "HERBATA", "rower w garażu"], 1..12), trust in proptest::collection::vec(any::<bool>(), 12)) {
        let entries: Vec<MemoryEntry> = texts.iter().enumerate().map(|(i, t)| {
            let p = if trust[i] { Provenance::User } else { Provenance::Agent { agent: AgentId::new("beta") } };
            entry(&format!("e{i}"), t, i64::try_from(i).unwrap(), p)
        }).collect();
        let mut seen = std::collections::BTreeSet::new();
        for op in duplicates(&entries, now(), &ConsolidationConfig::default()) {
            let ChangeOp::Merge { keep, duplicates, .. } = op else { panic!("tylko scalenia") };
            prop_assert!(!duplicates.contains(&keep));
            let rank = |id: &MemoryId| entries.iter().find(|e| &e.id == id).map(|e| memory_contract::trust_rank(&e.provenance)).unwrap();
            prop_assert!(duplicates.iter().all(|d| rank(d) <= rank(&keep)));
            for id in duplicates.iter().chain(std::iter::once(&keep)) {
                prop_assert!(seen.insert(id.clone()), "grupy rozłączne");
            }
        }
    }
}
