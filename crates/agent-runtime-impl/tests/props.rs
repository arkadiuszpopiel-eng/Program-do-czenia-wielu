//! Własności runtime (ACC-F3-agent-runtime-03): dla losowych skryptów modelu replay dziennika
//! odtwarza stan, liczba kroków w zdarzeniach = zużycie w checkpoincie, każda historia wysłana
//! do modelu jest poprawna (każde wywołanie narzędzia ma wynik), przebieg zawsze się kończy.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use agent_runtime_contract::{AgentRuntime, CheckpointStore, RunEvent, RunStatus};
use proptest::prelude::*;
use serde_json::json;

#[derive(Debug, Clone)]
enum Turn {
    Call(u8, u8),
    Answer,
}

fn turn() -> impl Strategy<Value = Turn> {
    prop_oneof![
        4 => (0u8..6, 0u8..4).prop_map(|(t, p)| Turn::Call(t, p)),
        1 => Just(Turn::Answer),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn replay_reconstructs_state(turns in proptest::collection::vec(turn(), 1..12), max_steps in 3u32..30) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().start_paused(true).build().unwrap();
        rt.block_on(async {
            let w = common::world();
            for (i, t) in turns.iter().enumerate() {
                let script = match t {
                    Turn::Call(tool, p) => common::call(&format!("t{i}"), common::TOOLS[usize::from(*tool)], json!({"path": format!("/Users/ala/p{p}"), "content": "x", "from": "/a", "to": "/b"})),
                    Turn::Answer => common::answer("Koniec."),
                };
                w.provider.push_script(script);
            }
            w.provider.push_script(common::answer("Koniec."));
            let mut spec = common::spec();
            spec.budget.max_steps = max_steps;
            let run = w.runtime.start(spec).await.unwrap();
            let outcome = w.runtime.wait(&run).await.unwrap();
            let events = w.runtime.events(&run).unwrap();
            let status = w.runtime.status(&run).unwrap();
            prop_assert_eq!(RunStatus::replay(events.iter().map(|e| &e.event)), status.clone());
            prop_assert_eq!(status, RunStatus::Finished { outcome });
            let started = events.iter().filter(|e| matches!(e.event, RunEvent::StepStarted { .. })).count();
            let cp = w.store.latest(&run).unwrap().unwrap();
            prop_assert_eq!(started as u32, cp.usage.steps);
            prop_assert!(cp.usage.steps <= max_steps);
            for r in w.provider.requests() {
                prop_assert!(r.validate().is_ok());
            }
            Ok(())
        })?;
    }
}
