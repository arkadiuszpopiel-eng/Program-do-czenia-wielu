//! Testy kontraktowe runtime na implementacji (model skryptowany, wirtualny zegar).

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::time::Duration;

use agent_runtime_contract::contract_tests as ct;
use serde_json::json;

#[tokio::test(start_paused = true)]
async fn finishing_run() {
    let w = common::world();
    w.provider
        .push_script(common::answer("Gotowe — folder był już uporządkowany."));
    ct::finishing_run_is_consistent(&w.runtime, common::spec()).await;
}

#[tokio::test(start_paused = true)]
async fn long_run_steer_and_cancel() {
    let w = common::world();
    for i in 0..500 {
        w.provider.push_script(
            common::call(
                &format!("t{i}"),
                "fs_list",
                json!({"path": format!("/Users/ala/d{i}")}),
            )
            .delayed(Duration::from_millis(50)),
        );
    }
    let mut spec = common::spec();
    spec.budget.max_steps = 10_000;
    ct::long_run_steer_and_cancel(&w.runtime, spec).await;
}

#[tokio::test]
async fn errors() {
    let w = common::world();
    ct::errors(&w.runtime, common::spec()).await;
}
