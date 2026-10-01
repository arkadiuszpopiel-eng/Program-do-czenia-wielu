# mcp-contract

Kontrakt modułu `mcp` (PLAN §8.7, §9.7; SPEC: `docs/modules/mcp/SPEC.md`): JSON-RPC 2.0 (MCP i tryb
bez nagłówka dla `codex app-server`), typy MCP 2025-06-18, odcisk definicji narzędzi (SHA-256
kanonicznego JSON), skaner prompt injection w opisach, poziomy zaufania i zgody (`assess`), trait
`McpClient`, serwer MCP Alfy v0 (`AlfaTool`: schowek, okna, `approve`), host mostów
(`BridgeMcpHost`, `ApprovalRouter`, token sesyjny z TTL, powitanie proxy) oraz wspólną obsługę
protokołu serwera (`ServerSession`). Feature `contract-tests`: zestaw dla hostów mostów.
