// Serwery MCP podłączane przez użytkownika (Ustawienia → Serwery MCP; komendy `mcp_*`).
// Wartości zmiennych tajnych nigdy nie wracają do interfejsu (tylko nazwy); opisy narzędzi
// pochodzą od serwera — treść niezaufana (pokazywana jako zwykły tekst).

export type {
  McpConnState,
  McpEnvVar,
  McpServerInput,
  McpServerTools,
  McpServerView,
  McpToolState,
  McpToolView,
  McpTrust,
} from './generated/ipc';
