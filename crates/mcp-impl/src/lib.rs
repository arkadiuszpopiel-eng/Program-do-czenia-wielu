//! Moduł `mcp` (docs/modules/mcp/SPEC.md, PLAN §8.7, §9.7, F4):
//! - [`StdioMcpClient`] — klient serwerów MCP uruchamianych przez stdio (odcisk opisów narzędzi,
//!   poziomy zaufania, blokada po zmianie opisu do ponownej zgody użytkownika);
//! - [`LocalMcpHost`] — serwer MCP Alfy v0 dla mostów CLI: narzędzia schowka i okien (porty
//!   `platform-contract`) + wewnętrzne `approve`; transport wyłącznie przez `alfa-mcp-proxy`
//!   (stdio) i kanał lokalny z tokenem sesyjnym z TTL; **bez nasłuchu sieciowego**.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod client;
mod connection;
mod host;
pub mod lines;
pub mod listener;
mod tools;

pub use client::{SERVER_ENV_ALLOWLIST, StdioMcpClient};
pub use host::{Clock, HostConfig, LocalMcpHost, MonotonicClock};
pub use tools::{AlfaToolHandler, PlatformPorts};

/// Treść `module.toml` tego modułu.
pub const MODULE_TOML: &str = include_str!("../module.toml");
