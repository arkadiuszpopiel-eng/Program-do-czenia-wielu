// Serwery MCP podłączane przez użytkownika (Ustawienia → Serwery MCP; komendy `mcp_*`).
// Wartości zmiennych tajnych nigdy nie wracają do interfejsu (tylko nazwy); opisy narzędzi
// pochodzą od serwera — treść niezaufana (pokazywana jako zwykły tekst).

/** Zaufanie: każde narzędzie wymaga zgody / przejrzany / zaufany (auto-zgoda bez sygnałów). */
export type McpTrust = 'untrusted' | 'reviewed' | 'trusted';

/** Stan połączenia: wyłączony, czeka na pierwsze użycie, połączony, ostatnia próba nieudana. */
export type McpConnState = 'disabled' | 'idle' | 'connected' | 'failed';

export interface McpEnvVar {
  readonly name: string;
  /** Wartość jawna; `null` dla tajnej (Menedżer poświadczeń Windows). */
  readonly value: string | null;
  readonly secret: boolean;
}

export interface McpServerView {
  readonly id: string;
  readonly name: string;
  readonly command: string;
  /** Pełna ścieżka znalezionego programu; `null` — nie znaleziono w PATH. */
  readonly resolved: string | null;
  readonly args: readonly string[];
  readonly cwd: string | null;
  readonly env: readonly McpEnvVar[];
  readonly trust: McpTrust;
  readonly enabled: boolean;
  readonly state: McpConnState;
  readonly error: string | null;
  readonly server_info: string | null;
  readonly tools_total: number;
  readonly tools_approved: number;
  readonly timeout_ms: number;
}

/** Formularz serwera (`id: null` — nowy). Tajna zmienna z `value: null` zachowuje zapisaną. */
export interface McpServerInput {
  readonly id: string | null;
  readonly name: string;
  readonly command: string;
  readonly args: readonly string[];
  readonly cwd: string | null;
  readonly env: readonly McpEnvVar[];
  readonly trust: McpTrust;
  readonly enabled: boolean;
  readonly timeout_ms: number | null;
}

/** Zgoda: użytkownika / automatyczna / nowe / opis zmieniony po zgodzie (zablokowane). */
export type McpToolState = 'approved' | 'auto_approved' | 'new' | 'changed';

export interface McpToolView {
  readonly name: string;
  readonly title: string | null;
  readonly description: string | null;
  readonly state: McpToolState;
  readonly fingerprint: string;
  readonly suspicious: boolean;
  readonly params: readonly string[];
}

export interface McpServerTools {
  readonly server: McpServerView;
  readonly tools: readonly McpToolView[];
}
