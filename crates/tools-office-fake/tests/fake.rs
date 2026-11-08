//! Atrapa przechodzi testy kontraktowe, oznacza odczyty jako niezaufane i odrzuca edycje
//! niezgodne z kontraktem (formuły spoza listy) bez wykonania.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use tools_common_contract::contract_tests::ctx;
use tools_common_contract::{ToolOutcome, Toolset};
use tools_office_fake::FakeTools;

#[tokio::test]
async fn contract_and_scripting() {
    let fake = FakeTools::default();
    tools_office_contract::contract_tests::run_all(&fake.tools()).await;
    let read = fake.tools().remove(0);
    let name = read.manifest().name.clone();
    fake.push(
        &name,
        ToolOutcome::ok("skrypt", serde_json::json!({"x": 1})),
    );
    let out = read
        .call(tools_office_contract::sample_args(&name), &ctx("/"))
        .await;
    assert_eq!(out.text, "skrypt");
    assert_eq!(out.untrusted, read.manifest().untrusted_output.clone());
    let edit = fake.tools().remove(1);
    let bad = serde_json::json!({"path": "/a.xlsx", "edits": [
        {"op": "set_cells", "start": "A1", "rows": [["=WEBSERVICE(\"http://x\")"]]}]});
    assert!(!edit.call(bad, &ctx("/")).await.is_ok());
    assert!(fake.calls().iter().any(|(t, _)| *t == name));
}
