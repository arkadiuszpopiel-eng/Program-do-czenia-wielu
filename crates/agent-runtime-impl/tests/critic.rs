//! v1: Krytyczka zamiast samoweryfikacji — osobna rola tylko do odczytu, inna persona niż
//! autorka (`Cast::verifier_for`), wynik autorki jako dane; odrzucenie → poprawka → akceptacja;
//! zastępczyni, gdy Krytyczka jest autorką; samoweryfikacja v0 tylko w obsadzie Solo.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{Arc, Mutex};

use agent_runtime_contract::{
    AgentRuntime, MemCheckpointStore, RunEvent, RunOptions, RunOutcome, RunSpec,
};
use agent_runtime_impl::{Runtime, RuntimeConfig, RuntimeDeps};
use common::v1::{Routed, Slow, spec_for, standard_crew};
use common::{answer, call};
use personas_contract::{Cast, PersonaId, builtin_roles};
use serde_json::json;
use tools_common_contract::Tool;
use tools_fs_contract::FsToolKind;

fn world() -> (Routed, Runtime) {
    let p = Routed::new(&["m-delta", "m-gama", "m-alfa"]);
    let spans = Arc::new(Mutex::new(Vec::new()));
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(Slow::new(FsToolKind::Write, 10, spans.clone())),
        Arc::new(Slow::new(FsToolKind::Read, 10, spans)),
    ];
    let rt = Runtime::new(RuntimeDeps {
        provider: Arc::new(p.clone()),
        tools,
        checkpoints: Arc::new(MemCheckpointStore::default()),
        bus: None,
        config: RuntimeConfig::default(),
    });
    (p, rt)
}

fn author(persona: &str, role: &str) -> RunSpec {
    let mut s = spec_for(persona, role, &["fs_read", "fs_write"]);
    s.verify = true;
    s
}

fn crew_options() -> RunOptions {
    RunOptions {
        crew: Some(standard_crew(&[("critic", "m-gama")])),
        ..RunOptions::default()
    }
}

fn verdicts(rt: &Runtime, run: &agent_runtime_contract::RunId) -> Vec<(bool, String)> {
    rt.events(run)
        .unwrap()
        .iter()
        .filter_map(|e| match &e.event {
            RunEvent::Verified { ok, note } => Some((*ok, note.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn critic_rejects_then_author_fixes_then_accepts() {
    let (p, rt) = world();
    let d = p.model("m-delta");
    d.push_script(call(
        "w1",
        "fs_write",
        json!({"path": "/u/raport.md", "content": "v1"}),
    ));
    d.push_script(answer("Gotowe: raport zapisany."));
    d.push_script(call(
        "w2",
        "fs_write",
        json!({"path": "/u/raport.md", "content": "v2 + Podsumowanie"}),
    ));
    d.push_script(answer("Poprawione: dodałam podsumowanie."));
    let g = p.model("m-gama");
    g.push_script(call("r1", "fs_read", json!({"path": "/u/raport.md"})));
    g.push_script(answer("WERYFIKACJA: BŁĄD — brak sekcji Podsumowanie."));
    g.push_script(answer("WERYFIKACJA: OK — raport kompletny."));
    let run = rt
        .start_with(author("delta", "operator"), crew_options())
        .await
        .unwrap();
    assert_eq!(
        rt.wait(&run).await.unwrap(),
        RunOutcome::Completed {
            summary: "Poprawione: dodałam podsumowanie.".into(),
            verified: Some(true)
        }
    );
    let v = verdicts(&rt, &run);
    assert_eq!(
        v.iter().map(|(ok, _)| *ok).collect::<Vec<_>>(),
        vec![false, true]
    );
    assert!(v[0].1.starts_with("Gama (Krytyczka)") && v[0].1.contains("Podsumowanie"));
    let kids = rt.children(&run).unwrap();
    assert_eq!(kids.len(), 2, "każda runda = osobny przebieg Krytyczki");
    for k in &kids {
        let ev = rt.events(k).unwrap();
        let RunEvent::Started { tools, goal, .. } = &ev[0].event else {
            panic!()
        };
        assert_eq!(tools, &vec!["fs_read".to_owned()], "Krytyczka tylko odczyt");
        assert!(goal.contains("<<<NIEZAUFANE") && goal.contains("WERYFIKACJA"));
    }
    for req in g.requests() {
        assert!(
            req.system.as_deref().unwrap().contains("Gama"),
            "Krytyczka ≠ autorka"
        );
        assert!(req.tools.iter().all(|t| t.name != "fs_write"));
    }
    let fix_request = &d.requests()[2];
    assert!(
        fix_request
            .messages
            .iter()
            .any(|m| m.visible_text().contains("[Krytyczka] Wykryto problem"))
    );
    assert!(
        d.requests().iter().all(|r| !r
            .messages
            .iter()
            .any(|m| m.visible_text().contains("[Weryfikacja] Sprawdź"))),
        "bez samoweryfikacji"
    );
    let report = rt.report(&run).unwrap();
    assert_eq!(report.verification.len(), 2);
    assert_eq!(report.children.len(), 2);
}

#[tokio::test(start_paused = true)]
async fn critic_who_is_the_author_gets_a_substitute() {
    let (p, rt) = world();
    p.model("m-gama").push_script(answer("Analiza gotowa."));
    p.model("m-alfa").push_script(answer("WERYFIKACJA: OK"));
    let mut opts = crew_options();
    opts.crew.as_mut().unwrap().models.clear();
    let mut s = author("gama", "critic");
    s.model = "m-gama".into();
    let crew = opts.crew.as_mut().unwrap();
    crew.models
        .insert(personas_contract::RoleId::critic(), "m-alfa".into());
    let run = rt.start_with(s, opts).await.unwrap();
    assert!(matches!(
        rt.wait(&run).await.unwrap(),
        RunOutcome::Completed {
            verified: Some(true),
            ..
        }
    ));
    let v = verdicts(&rt, &run);
    assert!(v[0].1.starts_with("Alfa (Krytyczka, zastępstwo)"), "{v:?}");
}

#[tokio::test(start_paused = true)]
async fn solo_cast_falls_back_to_self_check() {
    let (p, rt) = world();
    let d = p.model("m-delta");
    d.push_script(answer("Gotowe."));
    d.push_script(answer("WERYFIKACJA: OK"));
    let mut opts = crew_options();
    let crew = opts.crew.as_mut().unwrap();
    crew.cast = Cast::solo(
        PersonaId::delta(),
        builtin_roles().into_iter().map(|r| r.id),
        false,
    );
    let run = rt
        .start_with(author("delta", "operator"), opts)
        .await
        .unwrap();
    assert!(matches!(
        rt.wait(&run).await.unwrap(),
        RunOutcome::Completed {
            verified: Some(true),
            ..
        }
    ));
    assert!(rt.children(&run).unwrap().is_empty());
    assert!(p.model("m-gama").requests().is_empty());
}

#[tokio::test(start_paused = true)]
async fn persistent_rejection_ends_unverified() {
    let (p, rt) = world();
    for i in 0..3 {
        p.model("m-delta")
            .push_script(answer(&format!("Wersja {i}")));
        p.model("m-gama")
            .push_script(answer("WERYFIKACJA: BŁĄD — nadal źle"));
    }
    let run = rt
        .start_with(author("delta", "operator"), crew_options())
        .await
        .unwrap();
    let out = rt.wait(&run).await.unwrap();
    assert!(
        matches!(
            out,
            RunOutcome::Completed {
                verified: Some(false),
                ..
            }
        ),
        "{out:?}"
    );
    assert_eq!(rt.children(&run).unwrap().len(), 2, "≤ max_verify_rounds");
}
