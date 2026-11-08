//! Atrapy modułu `mcp` (docs/modules/mcp/SPEC.md „Fake”):
//! - [`FakeMcpServer`] — zewnętrzny serwer MCP w procesie (duplex) ze sterowanymi narzędziami:
//!   zmiana opisu po zatwierdzeniu (z powiadomieniem albo bez), złośliwe opisy, rejestr wywołań;
//! - [`FakeBridgeMcpHost`] — host MCP mostów z prawdziwym kanałem lokalnym i ręcznym zegarem TTL
//!   (dla testów `agent-backends`, które nie mogą zależeć od `mcp-impl`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod host;
mod server;

pub use host::FakeBridgeMcpHost;
pub use server::{FakeMcpServer, simple_tool};
