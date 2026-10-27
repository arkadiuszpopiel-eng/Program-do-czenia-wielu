//! Silnik tury czatu Alfy (kategoria `app-*`, wydzielony z `app-core` — limit rozmiaru crate'a):
//! tura użytkownika (append-only) → generacja (strumień dostawcy renderowany przyrostowo w Rust,
//! przebieg agentki z narzędziami albo delegacja do mostu CLI) → zapis tury agentki, koszt w
//! `cost-meter`, fakty tury, oś czasu, zdarzenia UI. Najwyżej jedna generacja na sesję.
//! `app-core` składa silnik (`ChatDeps`), wiąże go z rdzeniem (`ChatHost`) i wystawia komendy.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod access;
mod agent;
mod arena;
mod checkpoints;
mod child_state;
mod conductor;
mod conductor_budget;
mod conductor_child;
mod conductor_pipeline;
mod conductor_record;
mod conductor_spec;
mod conductor_state;
mod conductor_stream;
mod conductor_summary;
mod delegate;
mod engine;
mod finish;
mod generate;
mod history;
mod offline;
mod outcome;
pub mod pipelines;
mod project;
mod quick;
mod resume;
mod routing;
pub mod session_cards;
mod settings;
mod stage_route;
mod stream;
mod style;
mod taint;
mod undo;

pub use arena::{ARENA_MODELS, arena_model, arena_placement};
pub use engine::{AgentStack, ChatDeps, ChatEngine, ChatHost, GenHandle, RunCtl};
pub use generate::{GenRequest, Placement};
pub use history::CONTINUE_HINT;
pub use offline::{QueueEntry, requeue};
pub use project::{author_of, block_dto, render_closed, turn_dto};

mod agent_setup;

mod child_resolve;

mod conductor_finish;

mod conductor_init;

mod project_context;

mod conductor_evidence;
mod conductor_handoff;
