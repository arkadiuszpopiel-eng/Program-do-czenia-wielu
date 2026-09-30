//! Testy atrapy: kontrakt współdzielony + rejestrowanie i wstrzykiwanie błędów.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use example_module_contract::{contract_tests, Echo, EchoError};
use example_module_fake::FakeEcho;

#[tokio::test]
async fn contract_suite() {
    contract_tests::run_all(|| async { FakeEcho::new() }).await;
}

#[tokio::test]
async fn records_calls_and_injects_failure() {
    let fake = FakeEcho::new();
    fake.echo("a").await.unwrap();
    fake.fail_next(EchoError::NotStarted);
    assert_eq!(fake.echo("b").await, Err(EchoError::NotStarted));
    let ok = fake.echo("c").await.unwrap();
    assert_eq!(ok.seq, 2);
    assert_eq!(fake.calls(), vec!["a", "b", "c"]);
}
