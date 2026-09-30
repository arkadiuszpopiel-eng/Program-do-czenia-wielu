//! Testy atrapy: kontrakt współdzielony + determinizm i licznik kaskady.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use memory_contract::contract_tests;
use memory_contract::{Memory, MemoryScope, NewMemory, RememberMode, SessionId};
use memory_fake::FakeMemory;

#[test]
fn contract_suite() {
    contract_tests::run_all(|| Box::new(FakeMemory::new()));
}

#[test]
fn deterministic_ids_and_forget_counter() {
    let a = FakeMemory::new();
    let b = FakeMemory::new();
    let fact = || NewMemory::user_fact(SessionId::new("s"), "fakt");
    let ea = a.remember(fact(), RememberMode::Explicit).unwrap();
    let eb = b.remember(fact(), RememberMode::Explicit).unwrap();
    assert_eq!(ea, eb);
    assert_eq!(ea.id.0, "mem-0001");
    let scope = MemoryScope::Session(SessionId::new("s"));
    a.forget(&scope, &ea.id).unwrap();
    assert_eq!(a.forget_count(), 1);
    assert!(a.list(&scope).unwrap().is_empty());
}
