//! Serwery MCP podłączane przez użytkownika (Ustawienia → Serwery MCP) i narzędzia agentek nad
//! nimi. Serwer to program uruchamiany przez stdio (np. `npx -y <pakiet>`, `uvx <pakiet>`) z czystym
//! środowiskiem: lista dozwolonych zmiennych systemowych, zmienne jawne z ustawień i zmienne tajne
//! z Menedżera poświadczeń. Agentka widzi tylko narzędzia z aktualną zgodą użytkownika (zmiana
//! opisu po zgodzie blokuje narzędzie), a każde wywołanie przechodzi przez Brokera.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

mod edit;
mod hub;
mod store;
mod tools;
mod views;

pub use hub::{Connector, McpHub, McpHubDeps, StdioConnector};
pub use store::{FilePinStore, PINS_FILE, SERVERS_FILE, ServerEntry, looks_secret, slug};
pub use tools::{McpCallTool, McpToolsTool, agent_tools};
