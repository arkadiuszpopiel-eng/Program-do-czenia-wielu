//! Silnik tury czatu Alfy (kategoria `app-*`, wydzielony z `app-core` — limit rozmiaru crate'a):
//! tura użytkownika (append-only) → generacja (strumień dostawcy renderowany przyrostowo w Rust,
//! przebieg agentki z narzędziami albo delegacja do mostu CLI) → zapis tury agentki, koszt w
//! `cost-meter`, fakty tury, oś czasu, zdarzenia UI. Najwyżej jedna generacja na sesję.
//! `app-core` składa silnik (`ChatDeps`), wiąże go z rdzeniem (`ChatHost`) i wystawia komendy.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod agent;
mod delegate;
mod engine;
mod finish;
mod generate;
mod history;
mod outcome;
mod project;
mod routing;
mod stream;
mod taint;
mod undo;

pub use engine::{AgentStack, ChatDeps, ChatEngine, ChatHost, GenHandle, RunCtl};
pub use generate::{GenRequest, Placement};
pub use history::CONTINUE_HINT;
pub use project::{author_of, block_dto, render_closed, turn_dto};
