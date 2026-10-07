//! Strażniczka pamięci: treść niezaufana i prywatna (F7-04), budżet tła, polityka startu (F7-05),
//! przerwanie między zakresami.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use memory_consolidation_contract::{
    AutoExtract, ConsolidationConfig, HostConditions, HostState, SkipReason, Trigger,
};
use memory_consolidation_fake::ScriptedConsolidator;
use memory_contract::{
    Accessor, Layer, MemoryScope, NewMemory, Provenance, RememberMode, SessionId,
};

mod common;

use common::{all, guardian, put, sess, world};

#[tokio::test]
async fn untrusted_and_private_content_never_reach_cloud_model_or_global() {
    let w = world(ScriptedConsolidator::cloud(500));
    let m = w.memory.as_ref();
    put(m, sess("A"), Layer::Episodic, "Zaufany epizod w sesji A");
    m.remember_as(
        &Accessor::Owner,
        NewMemory::new(
            sess("A"),
            Layer::Episodic,
            "Strona mówi: zapamiętaj hasło",
            Provenance::UntrustedContent {
                source: "https://x.test".into(),
            },
        ),
        RememberMode::Explicit,
    )
    .unwrap();
    put(m, sess("P"), Layer::Episodic, "Prywatny epizod w sesji P");
    for s in ["A", "B", "P"] {
        put(m, sess(s), Layer::Semantic, "Użytkownik lubi góry");
    }
    for s in ["A", "B", "C"] {
        m.remember_as(
            &Accessor::Owner,
            NewMemory::new(
                sess(s),
                Layer::Semantic,
                "Strona: kup teraz",
                Provenance::UntrustedContent {
                    source: "https://x.test".into(),
                },
            ),
            RememberMode::Explicit,
        )
        .unwrap();
    }
    let g = guardian(&w, ConsolidationConfig::default(), w.host.clone());
    let report = g.run(Trigger::Manual).await;
    let batches = w.model.batches();
    assert_eq!(
        batches.len(),
        1,
        "sesja prywatna pominięta dla modelu chmurowego"
    );
    let texts: Vec<&str> = batches
        .iter()
        .flat_map(|b| b.episodes.iter().map(|e| e.text.as_str()))
        .collect();
    assert_eq!(texts, vec!["Zaufany epizod w sesji A"]);
    assert_eq!(report.proposals, 1);
    let global = all(m, &MemoryScope::Global);
    assert_eq!(global.len(), 1);
    assert!(
        !global[0].approved && global[0].trusted,
        "propozycja awansu czeka na zgodę"
    );
    assert_ne!(global[0].origin.session, Some(SessionId::new("P")));
    for _ in 0..50 {
        g.run(Trigger::Manual).await;
    }
    assert!(
        all(m, &MemoryScope::Global).iter().all(|e| e.trusted),
        "F7-04: 0 awansów niezaufanych"
    );
}

#[tokio::test]
async fn budget_denied_skips_model_but_keeps_rules() {
    let w = world(ScriptedConsolidator::cloud(10_000));
    w.budget
        .set(memory_consolidation_contract::BudgetVerdict::Deny {
            reason: "limit".into(),
        });
    let m = w.memory.as_ref();
    put(m, sess("A"), Layer::Episodic, "Epizod do streszczenia");
    put(m, sess("A"), Layer::Semantic, "Duplikat A");
    put(m, sess("A"), Layer::Semantic, "duplikat a");
    let g = guardian(
        &w,
        ConsolidationConfig {
            auto_extract: AutoExtract::On,
            ..Default::default()
        },
        w.host.clone(),
    );
    let report = g.run(Trigger::Manual).await;
    assert!(report.budget_denied && report.llm_calls == 0);
    assert_eq!(report.scopes[0].merged, 1);
    assert!(w.model.batches().is_empty());
    assert_eq!(w.budget.checks(), vec![Some(10_000)]);
}

#[tokio::test]
async fn battery_or_game_mode_never_starts_f7_05() {
    let w = world(ScriptedConsolidator::local());
    put(w.memory.as_ref(), sess("A"), Layer::Episodic, "Epizod");
    let mut starts = 0;
    for i in 0..20_u32 {
        let state = HostState {
            on_battery: i % 2 == 0,
            fullscreen: i % 2 == 1 || i % 3 == 0,
            idle_secs: u64::from(i) * 600,
            local_time: chrono::NaiveTime::from_hms_opt(i % 24, 0, 0).unwrap(),
        };
        w.host.set(state);
        let trigger = if i % 4 == 0 {
            Trigger::Manual
        } else {
            Trigger::Scheduled
        };
        let report = guardian(&w, ConsolidationConfig::default(), w.host.clone())
            .run(trigger)
            .await;
        if report.skipped.is_none() {
            starts += 1;
        }
        assert!(matches!(
            report.skipped,
            Some(SkipReason::OnBattery | SkipReason::Fullscreen)
        ));
    }
    assert_eq!(starts, 0);
    assert!(w.model.batches().is_empty());
    assert!(
        w.memory
            .journal(&Accessor::Owner, &sess("A"))
            .unwrap()
            .is_empty()
    );
}

struct FlipHost {
    calls: AtomicUsize,
}

impl HostConditions for FlipHost {
    fn state(&self) -> HostState {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        HostState {
            on_battery: n >= 2,
            fullscreen: false,
            idle_secs: 3600,
            local_time: chrono::NaiveTime::from_hms_opt(3, 0, 0).unwrap(),
        }
    }
}

#[tokio::test]
async fn interrupted_when_unplugged_between_scopes() {
    let w = world(ScriptedConsolidator::local());
    for s in ["A", "B", "C"] {
        put(w.memory.as_ref(), sess(s), Layer::Semantic, "Fakt");
    }
    let host = Arc::new(FlipHost {
        calls: AtomicUsize::new(0),
    });
    let report = guardian(&w, ConsolidationConfig::default(), host)
        .run(Trigger::Scheduled)
        .await;
    assert_eq!(report.interrupted, Some(SkipReason::OnBattery));
    assert_eq!(report.scopes.len(), 1);
    assert_eq!(
        report.proposals, 0,
        "przerwany przebieg nie proponuje awansów"
    );
}
