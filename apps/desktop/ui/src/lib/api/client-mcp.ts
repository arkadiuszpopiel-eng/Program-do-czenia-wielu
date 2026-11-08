// Interfejs serwerów MCP użytkownika (część `AlfaClient`; komendy `mcp_*` w COMMANDS.md). Zgody na
// narzędzia daje wyłącznie użytkownik na stronie Ustawienia → Serwery MCP.
import type { McpServerInput, McpServerTools, McpServerView } from './types-mcp';

export interface McpApi {
  list(): Promise<readonly McpServerView[]>;
  /** Dodanie albo zmiana; wartości tajne trafiają do Menedżera poświadczeń. */
  save(input: McpServerInput): Promise<McpServerView>;
  remove(id: string): Promise<void>;
  /** Uruchamia serwer (gdy włączony) i zwraca narzędzia ze stanem zgody. */
  tools(id: string): Promise<McpServerTools>;
  /** Zgoda na narzędzie w wersji opisu o podanym odcisku. */
  approve(id: string, tool: string, fingerprint: string): Promise<McpServerTools>;
  revoke(id: string, tool: string): Promise<McpServerTools>;
}
