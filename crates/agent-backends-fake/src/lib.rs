//! Atrapy modułu `agent-backends` (docs/modules/agent-backends/SPEC.md „Fake”):
//! - [`FakeAgentBackend`] — deterministyczny `AgentBackend` w pamięci (bez procesów), scenariusze
//!   ze znacznika w poleceniu (`contract_tests::scenario`), prawdziwe reguły pochodzenia;
//! - binarium `alfa-fake-agent-cli` — fałszywe CLI udające `claude -p` (stream-json, prośby
//!   o uprawnienia przez MCP Alfy) i `codex app-server` (JSON-RPC, zatwierdzenia serwera)
//!   do testów mostów od końca do końca bez prawdziwych CLI.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod backend;

pub use backend::FakeAgentBackend;
