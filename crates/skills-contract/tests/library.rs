//! Rdzeń biblioteki: walidacja (negatywne), zdarzenia bez treści, szkic z pamięci
//! proceduralnej, wyszukiwanie deterministyczne i port modelu (model nie wprowadza obcych
//! umiejętności), własności: uprawnienia ≤ roli, hash niezależny od kolejności pól.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use async_trait::async_trait;
use chrono::Utc;
use memory_contract::{Layer, MemoryEntry, MemoryId, MemoryScope, Provenance};
use personas_contract::builtin_roles;
use proptest::prelude::*;
use serde_json::json;
use skills_contract::samples::{sample_catalog, sample_skill};
use skills_contract::{
    ApprovalOrigin, OwnerApproval, SkillError, SkillId, SkillLibrary, SkillMatch, SkillRanker,
    SkillSource, SkillState, content_hash, draft_from_memory, rerank, runnable_by, search,
};

fn lib() -> SkillLibrary {
    SkillLibrary::new(sample_catalog(), Vec::new())
}

type Mutation = Box<dyn Fn(&mut skills_contract::Skill)>;

#[test]
fn invalid_skills_are_rejected() {
    let cases: Vec<(&str, Mutation)> = vec![
        ("id", Box::new(|s| s.id = SkillId::new("Zła Nazwa"))),
        ("opis", Box::new(|s| s.description = "krótko".into())),
        (
            "narzędzie",
            Box::new(|s| s.required_tools.push("shell_run".into())),
        ),
        (
            "Jądro",
            Box::new(|s| s.required_capabilities.push("system.admin".into())),
        ),
        (
            "sekrety",
            Box::new(|s| s.required_capabilities = vec!["secrets.read".into()]),
        ),
        (
            "niezgodne zdolności",
            Box::new(|s| s.required_capabilities = vec!["fs.read".into()]),
        ),
        ("bez testów", Box::new(|s| s.acceptance.clear())),
        (
            "szablon",
            Box::new(|s| s.prompt = "Zrób {{nieznany}}".into()),
        ),
        (
            "schemat",
            Box::new(|s| s.parameters = json!({"type": "object"})),
        ),
        (
            "test nie przechodzi",
            Box::new(|s| s.acceptance[0].expect_in_goal.push("brak".into())),
        ),
        (
            "powtórzone",
            Box::new(|s| s.required_tools.push("fs_list".into())),
        ),
    ];
    for (name, mutate) in cases {
        let mut s = sample_skill("1.0.0");
        mutate(&mut s);
        assert!(lib().propose(s, SkillSource::User, 1).is_err(), "{name}");
    }
}

#[test]
fn events_carry_no_content_and_approval_installs() {
    let mut l = lib();
    let (r, ev) = l
        .propose(sample_skill("1.0.0"), SkillSource::User, 5)
        .unwrap();
    assert_eq!(ev.len(), 1);
    let payload = ev[0].payload.to_string();
    assert!(!payload.contains("Uporządkuj") && payload.contains(&r.hash));
    let a = OwnerApproval {
        origin: ApprovalOrigin::Ui,
        reviewed_hash: r.hash.clone(),
    };
    let (inst, ev) = l.approve(&r.skill.id, &r.skill.version, a, 9).unwrap();
    assert_eq!(
        (inst.state, inst.decided_at_ms),
        (SkillState::Installed, Some(9))
    );
    assert_eq!(ev[0].kind.as_str(), "skills.installed");
    assert!(matches!(
        l.reject(&r.skill.id, &r.skill.version, 10),
        Err(SkillError::WrongState(_))
    ));
}

fn entry(text: &str, trusted: bool, layer: Layer) -> MemoryEntry {
    MemoryEntry {
        id: MemoryId("m1".into()),
        scope: MemoryScope::Global,
        layer,
        text: text.into(),
        entities: vec![],
        provenance: if trusted {
            Provenance::User
        } else {
            Provenance::UntrustedContent {
                source: "www".into(),
            }
        },
        trusted,
        confidence: 1.0,
        ttl_secs: None,
        created_at: Utc::now(),
        approved: true,
        subject: None,
        origin: Default::default(),
        version: 1,
        supersedes: None,
        superseded: None,
        pinned: false,
        consolidated_at: None,
    }
}

#[test]
fn draft_from_procedural_memory() {
    let text = "Kopia zdjęć: \n1. fs_list w Obrazach\n2. fs_move do Archiwum\n3. sprawdź liczbę";
    let (skill, source) =
        draft_from_memory(&entry(text, true, Layer::Procedural), &sample_catalog()).unwrap();
    assert_eq!(skill.name, "Kopia zdjęć");
    assert_eq!(skill.steps.len(), 3);
    assert_eq!(
        skill.required_tools,
        vec!["fs_list".to_owned(), "fs_move".to_owned()]
    );
    assert!(matches!(source, SkillSource::Memory { trusted: true, .. }));
    let (r, _) = lib().propose(skill.clone(), source, 1).unwrap();
    assert_eq!(r.state, SkillState::Proposed);
    let (skill, source) =
        draft_from_memory(&entry(text, false, Layer::Procedural), &sample_catalog()).unwrap();
    assert_eq!(
        lib().propose(skill, source, 1).unwrap().0.state,
        SkillState::Quarantined
    );
    assert!(draft_from_memory(&entry(text, true, Layer::Semantic), &sample_catalog()).is_err());
}

struct Adversary;

#[async_trait]
impl SkillRanker for Adversary {
    async fn rank(&self, _task: &str, c: &[SkillMatch]) -> Result<Vec<(SkillId, f32)>, String> {
        let mut out = vec![
            (SkillId::new("wstrzyknieta"), 1.0),
            (SkillId::new("x"), f32::NAN),
        ];
        out.extend(c.iter().rev().map(|m| (m.id.clone(), 7.0)));
        Ok(out)
    }
}

struct Broken;

#[async_trait]
impl SkillRanker for Broken {
    async fn rank(&self, _t: &str, _c: &[SkillMatch]) -> Result<Vec<(SkillId, f32)>, String> {
        Err("model niedostępny".into())
    }
}

#[tokio::test]
async fn search_and_rerank() {
    let mut l = lib();
    let mut other = sample_skill("1.0.0");
    other.id = SkillId::new("raport-tygodniowy");
    other.name = "Raport tygodniowy".into();
    other.keywords = vec!["raport".into()];
    for s in [sample_skill("1.0.0"), other] {
        let (r, _) = l.propose(s, SkillSource::User, 1).unwrap();
        l.approve(
            &r.skill.id,
            &r.skill.version,
            OwnerApproval {
                origin: ApprovalOrigin::Ui,
                reviewed_hash: r.hash,
            },
            2,
        )
        .unwrap();
    }
    let ops: Vec<_> = builtin_roles()
        .into_iter()
        .filter(|r| r.id.as_str() == "operator")
        .collect();
    let found = search(
        l.records(),
        l.catalog(),
        &ops,
        "posortuj pobrane pliki w folderze",
        5,
    );
    assert_eq!(found[0].id.as_str(), "porzadki-pobranych");
    assert!(found[0].matched.contains(&"pobra".to_owned()));
    assert!(search(l.records(), l.catalog(), &ops, "", 5).is_empty());
    assert!(search(l.records(), l.catalog(), &ops, "pogoda w Krakowie", 5).is_empty());
    let both = search(l.records(), l.catalog(), &ops, "folder raport pobrane", 5);
    let reranked = rerank(&Adversary, "x", both.clone()).await;
    let ids = |v: &[SkillMatch]| {
        v.iter()
            .map(|m| m.id.clone())
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(
        ids(&reranked),
        ids(&both),
        "model nie wprowadza obcych umiejętności"
    );
    assert!(reranked.iter().all(|m| (0.0..=1.0).contains(&m.score)));
    assert_eq!(rerank(&Broken, "x", both.clone()).await, both);
}

proptest! {
    /// Umiejętność uruchamialna przez rolę ⇔ każde jej narzędzie jest dozwolone dla tej roli.
    #[test]
    fn runnable_only_within_role(tools in proptest::collection::btree_set(prop_oneof![Just("fs_list"), Just("fs_read"), Just("fs_move"), Just("fs_write")], 0..4), role_ix in 0usize..9) {
        let role = builtin_roles().remove(role_ix);
        let mut s = sample_skill("1.0.0");
        s.required_tools = tools.iter().map(|t| (*t).to_owned()).collect();
        let catalog = sample_catalog();
        let allowed = s.required_tools.iter().all(|t| {
            catalog.iter().find(|m| &m.name == t).is_some_and(|m| m.allowed_for(&role.tools, role.read_only))
        });
        prop_assert_eq!(runnable_by(&s, &catalog, std::slice::from_ref(&role)).is_ok(), allowed);
        if role.read_only {
            prop_assert!(!s.required_tools.iter().any(|t| t == "fs_move" || t == "fs_write") || runnable_by(&s, &catalog, &[role]).is_err());
        }
    }

    /// Hash nie zależy od kolejności kluczy w schemacie parametrów.
    #[test]
    fn hash_ignores_key_order(swap in any::<bool>()) {
        let a = sample_skill("1.0.0");
        let mut b = a.clone();
        b.parameters = if swap {
            json!({"additionalProperties": false, "required": ["folder"], "properties": {"tryb": {"default": "typ", "enum": ["typ", "data"], "type": "string"}, "folder": {"maxLength": 200, "type": "string"}}, "type": "object"})
        } else { a.parameters.clone() };
        prop_assert_eq!(content_hash(&a).unwrap(), content_hash(&b).unwrap());
    }
}
