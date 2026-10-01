//! Agentki z narzędziami w aplikacji (kategoria `app-*`, wydzielona z `app-core` — limit
//! rozmiaru crate'a):
//! - [`AgentTools`] — zestawy `tools-fs` / `tools-shell` / `tools-clipboard` nad jednym Brokerem
//!   (zwykle [`TicketLog`]) i dziennikiem cofania, filtr narzędzi rolami obsady;
//! - [`TicketLog`] — przezroczysty dekorator Brokera zapamiętujący fakty próśb o zatwierdzenie
//!   (karta „czeka na zatwierdzenie");
//! - [`run_spec`] / [`AgentSettings`] — `RunSpec` z obsady i Ustawień → Agentki (budżety kroków,
//!   czasu i kosztu w PLN, limit czekania na zatwierdzenie, samoweryfikacja);
//! - [`RunHandle`] / [`RunFeed`] — przebieg `agent-runtime` na dostawcy z Routera i jego dziennik
//!   na żywo bez luk (steering, anulowanie);
//! - [`RunProjector`] — zdarzenia `agent.*` → Replay, linie kroków w wątku, karty „Cofnij",
//!   „czeka na zatwierdzenie", „uruchom w terminalu", kapsuła aktywności, Oś czasu;
//! - [`eval`] — zestaw ewaluacyjny narzędzi F3 (`evals/F3/tools/`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod eval;
mod map;
mod project;
mod runner;
mod spec;
mod tickets;
mod toolset;

pub use map::{FinalText, final_text, short};
pub use project::{Projection, RunContext, RunProjector};
pub use runner::{RunFeed, RunHandle};
pub use spec::{AgentSettings, NO_WINDOW_APPROVAL_CAP_S, SpecInput, keys, run_spec};
pub use tickets::{TicketLog, TicketNote};
pub use tools_clipboard_contract::ClipboardUndoError;
pub use tools_shell_contract::ShellToolsConfig;
pub use toolset::{AgentTools, ToolsDeps};

/// Manifesty modułów składanych przez ten crate (identyfikator → `module.toml`) — rejestr
/// `core-registry` w `app-core` liczy z nich kolejność startu i pokazuje zdrowie.
pub const MODULES: &[(&str, &str)] = &[
    ("tools-fs", tools_fs_impl::MODULE_TOML),
    ("tools-shell", tools_shell_impl::MODULE_TOML),
    ("tools-clipboard", tools_clipboard_impl::MODULE_TOML),
    ("agent-runtime", agent_runtime_impl::MODULE_TOML),
];
