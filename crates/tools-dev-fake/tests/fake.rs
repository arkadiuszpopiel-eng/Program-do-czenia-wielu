//! Atrapa `tools-dev`: kontrakt, zapis wywołań, wynik skryptowany, odrzucenie wstrzyknięć.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use serde_json::json;
use tools_common_contract::{ToolOutcome, Toolset};
use tools_dev_fake::FakeTools;

#[tokio::test]
async fn contract_and_script() {
    let fake = FakeTools::default();
    tools_dev_contract::contract_tests::run_all(&fake.tools()).await;
    fake.push(
        "dev_test",
        ToolOutcome::ok("2 zaliczone", json!({"passed": 2})),
    );
    let test = fake
        .tools()
        .into_iter()
        .find(|t| t.manifest().name == "dev_test")
        .unwrap();
    let out = test
        .call(
            json!({"filter": "parse"}),
            &tools_common_contract::contract_tests::ctx("/"),
        )
        .await;
    assert_eq!(out.data["passed"], 2);
    assert!(fake.calls().iter().any(|(n, _)| n == "dev_test"));
}
