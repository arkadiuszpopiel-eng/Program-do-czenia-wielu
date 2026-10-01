//! Moduł `agent-backends` (docs/modules/agent-backends/SPEC.md, PLAN §5.1–5.2 klasa C, §8.5):
//! mosty do **oficjalnych, niezmodyfikowanych** CLI (Claude Code, Codex) jako „opaque worker”.
//!
//! - logowanie do CLI wykonuje wyłącznie użytkownik; ten crate nigdy nie czyta, nie kopiuje
//!   ani nie przechowuje tokenów CLI (test statyczny w `mcp-impl/tests/static_rules.rs`
//!   i `tests/compliance.rs`), a proces CLI dostaje środowisko bez kluczy Alfy;
//! - uruchomienie przechodzi przez [`gate`]: pochodzenie (tylko żądanie użytkownika; harmonogram
//!   z jawną zgodą; wyzwalacz i Ulepszacz — nigdy), trasa w `compliance`, przypięta wersja CLI;
//! - praca wyłącznie w worktree/kopii ([`GitWorkspace`]);
//! - prośby o uprawnienia → `ApprovalSink` (Claude: `--permission-prompt-tool` przez serwer MCP
//!   Alfy; Codex: zatwierdzenia `app-server`); zdarzenia oznaczone `unverified_by_alfa`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod approvals;
mod backend;
pub mod claude;
pub mod codex;
mod config;
pub mod gate;
mod lines;
mod log;
mod process;
mod runner;
mod workspace;

pub use backend::{BackendDeps, BridgeBackend};
pub use config::{BridgeConfig, BridgeProgram};
pub use process::{SystemTreeKiller, TreeKiller, cli_env};
pub use workspace::{DEFAULT_MAX_COPY_BYTES, GitWorkspace};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
