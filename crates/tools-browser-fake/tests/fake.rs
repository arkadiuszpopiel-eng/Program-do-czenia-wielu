//! Atrapa przechodzi testy kontraktowe, oznacza wyniki stron jako niezaufane i odrzuca
//! niedozwolone adresy bez wykonania.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use tools_browser_fake::FakeTools;
use tools_common_contract::contract_tests::ctx;
use tools_common_contract::{ToolOutcome, Toolset};

#[tokio::test]
async fn contract_and_scripting() {
    let fake = FakeTools::default();
    tools_browser_contract::contract_tests::run_all(&fake.tools()).await;
    let open = fake.tools().remove(0);
    fake.push(
        "browser_open",
        ToolOutcome::ok("Sklep", serde_json::json!({})),
    );
    let out = open
        .call(serde_json::json!({"url": "https://sklep.pl/"}), &ctx("/"))
        .await;
    assert_eq!(out.text, "Sklep");
    assert_eq!(out.untrusted, open.manifest().untrusted_output.clone());
    let bad = open
        .call(serde_json::json!({"url": "file:///C:/x"}), &ctx("/"))
        .await;
    assert!(!bad.is_ok());
    assert_eq!(
        fake.calls().len(),
        1,
        "zły adres odrzucony bez zapisu wywołania"
    );
}
