//! Broker — Safety Kernel, część 1: logika (docs/modules/safety-broker/SPEC.md, PLAN §8,
//! ADR 3, ADR 15). Usługa Windows na osobnym koncie, named pipe z ACL na SID i natywne okno
//! Broker-UI to część 2; tutaj silnik ([`BrokerEngine`]), Audyt z łańcuchem SHA-256 i kotwicą
//! ([`audit`]) oraz serwer/klient protokołu IPC na dowolnym strumieniu bajtów ([`ipc`]).
//!
//! Własności bezpieczeństwa: tokeny HMAC-SHA256 kluczem tylko w pamięci (rotacja, kasowanie
//! przy kill-switchu, nowy klucz i `boot` przy każdym starcie), potomek ⊆ rodzic, TTL zawsze
//! skończony, reguły Jądra sprawdzane przy wydaniu i przy każdym użyciu, każda decyzja w Audycie
//! (fail-closed: bez zapisu nie ma tokenu), zatwierdzenia wyłącznie z dowodem fizycznego wejścia.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod audit;
pub mod ipc;
pub mod service;

mod apply;
mod approvals;
mod broker;
mod decide;
mod engine;
mod keys;
mod kill;
mod state;
mod tokens;

pub use engine::{BrokerConfig, BrokerEngine};
pub use keys::KeyMode;

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
