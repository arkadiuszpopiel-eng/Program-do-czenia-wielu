//! Kontrakt modułu `mcp` (docs/modules/mcp/SPEC.md, PLAN §8.7, §9.7, F4).
//!
//! - [`jsonrpc`] — JSON-RPC 2.0 (MCP: ścisły; `codex app-server`: bez nagłówka `jsonrpc`);
//! - [`protocol`] — typy MCP **2025-06-18** (narzędzia, `initialize`);
//! - [`fingerprint`], [`injection`], [`trust`] — odcisk definicji narzędzia, skaner prompt
//!   injection, poziomy zaufania i zgody (zmiana opisu po zgodzie = blokada, S07/S08);
//! - [`client`] — trait [`McpClient`] i konfiguracja serwerów stdio;
//! - [`alfa`], [`bridge`], [`token`] — serwer MCP Alfy v0 (schowek, okna, `approve`) i v1 (UIA,
//!   zrzuty, rejestr tylko do odczytu — przez Brokera, wyniki `unverified_by_alfa`), host dla
//!   mostów: kanał lokalny (named pipe z ACL / gniazdo Unix 0600, **bez TCP**), token z TTL;
//! - [`server_core`] — obsługa protokołu po stronie serwera, wspólna dla `-impl` i `-fake`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod alfa;
pub mod alfa_v1;
pub mod bridge;
pub mod client;
mod error;
pub mod fingerprint;
pub mod injection;
pub mod jsonrpc;
pub mod protocol;
pub mod server_core;
pub mod token;
pub mod trust;

#[cfg(feature = "contract-tests")]
pub mod contract_tests;

pub use alfa::{
    ALFA_SERVER_NAME, AlfaTool, PERMISSION_PROMPT_TOOL, UNVERIFIED_FIELD, is_protected_process,
};
pub use alfa_v1::{ALFA_TOOLS_FINGERPRINT, alfa_tools_fingerprint};
pub use bridge::{
    ApprovalRouter, BridgeMcpHost, BridgeRegistration, BridgeScope, LocalEndpoint, McpServerLaunch,
    PermissionPromptRequest, PermissionPromptResponse, ProxyHello, RegistrationId, SessionToken,
};
pub use client::{McpClient, McpServerConfig, McpWarning};
pub use error::McpError;
pub use fingerprint::{ToolFingerprint, fingerprint};
pub use injection::{InjectionSignal, scan_tool};
pub use protocol::{CallToolResult, Content, Implementation, Tool};
pub use server_core::{ServerSession, ToolCallError, ToolHandler};
pub use token::{TokenRejection, TokenTable};
pub use trust::{
    ConsentOrigin, ConsentReason, MemoryPinStore, PinStore, ToolAssessment, ToolPin, ToolState,
    TrustLevel, assess,
};
