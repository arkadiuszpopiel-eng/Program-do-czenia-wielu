// Logika strony Ustawienia → Serwery MCP: gotowe przepisy popularnych serwerów (dla osoby, która
// nie zna poleceń), formularz ↔ dane komendy `mcp_save`, podsumowanie stanu serwera.
import type {
  McpEnvVar,
  McpServerInput,
  McpServerView,
  McpToolView,
  McpTrust,
} from '../api/types-mcp';

/** Czego potrzebuje przepis: Node.js (`npx`) albo uv (`uvx`). */
export type McpRuntime = 'node' | 'uv';

export interface McpPreset {
  readonly id: string;
  readonly name: string;
  /** Klucz opisu w i18n (`mcp.preset.<id>`). */
  readonly command: 'npx' | 'uvx';
  readonly args: readonly string[];
  /** Zmienne do uzupełnienia (tajne — wartość wpisuje użytkownik). */
  readonly env: readonly McpEnvVar[];
  /** Argument z folderem do wskazania przez użytkownika (`{folder}` w `args`). */
  readonly needsFolder: boolean;
  readonly runtime: McpRuntime;
}

const FOLDER = '{folder}';

/** Przepisy (oficjalne pakiety MCP i narzędzia przeglądarek). */
export const MCP_PRESETS: readonly McpPreset[] = [
  {
    id: 'playwright',
    name: 'Przeglądarka (Playwright)',
    command: 'npx',
    args: ['-y', '@playwright/mcp@latest'],
    env: [],
    needsFolder: false,
    runtime: 'node',
  },
  {
    id: 'chrome-devtools',
    name: 'Chrome DevTools',
    command: 'npx',
    args: ['-y', 'chrome-devtools-mcp@latest'],
    env: [],
    needsFolder: false,
    runtime: 'node',
  },
  {
    id: 'filesystem',
    name: 'Pliki w folderze',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-filesystem', FOLDER],
    env: [],
    needsFolder: true,
    runtime: 'node',
  },
  {
    id: 'git',
    name: 'Repozytorium Git',
    command: 'uvx',
    args: ['mcp-server-git', '--repository', FOLDER],
    env: [],
    needsFolder: true,
    runtime: 'uv',
  },
  {
    id: 'github',
    name: 'GitHub',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-github'],
    env: [{ name: 'GITHUB_PERSONAL_ACCESS_TOKEN', value: null, secret: true }],
    needsFolder: false,
    runtime: 'node',
  },
  {
    id: 'fetch',
    name: 'Pobieranie stron WWW',
    command: 'uvx',
    args: ['mcp-server-fetch'],
    env: [],
    needsFolder: false,
    runtime: 'uv',
  },
  {
    id: 'memory',
    name: 'Notatnik wiedzy',
    command: 'npx',
    args: ['-y', '@modelcontextprotocol/server-memory'],
    env: [],
    needsFolder: false,
    runtime: 'node',
  },
];

/** Wiersz zmiennej w formularzu (edytowalny). */
export interface EnvRow {
  name: string;
  value: string | null;
  secret: boolean;
}

/** Stan formularza (argumenty jako tekst — jeden argument w linii). */
export interface McpForm {
  id: string | null;
  name: string;
  command: string;
  argsText: string;
  cwd: string;
  env: EnvRow[];
  trust: McpTrust;
  enabled: boolean;
  timeoutSec: number;
}

export function emptyForm(): McpForm {
  return {
    id: null,
    name: '',
    command: '',
    argsText: '',
    cwd: '',
    env: [],
    trust: 'untrusted',
    enabled: true,
    timeoutSec: 30,
  };
}

/** Formularz z przepisu; `folder` zastępuje `{folder}` w argumentach. */
export function formFromPreset(p: McpPreset, folder = ''): McpForm {
  return {
    ...emptyForm(),
    name: p.name,
    command: p.command,
    argsText: p.args.map((a) => (a === FOLDER ? folder : a)).join('\n'),
    env: p.env.map((e) => ({ ...e })),
  };
}

/** Formularz z istniejącego serwera (wartości tajne puste — zostają zapisane). */
export function formFromServer(s: McpServerView): McpForm {
  return {
    id: s.id,
    name: s.name,
    command: s.command,
    argsText: s.args.join('\n'),
    cwd: s.cwd ?? '',
    env: s.env.map((e) => ({ ...e })),
    trust: s.trust,
    enabled: s.enabled,
    timeoutSec: Math.round(s.timeout_ms / 1000),
  };
}

/** Argumenty z tekstu: jeden w linii, puste linie pomijane, bez obcinania spacji wewnątrz. */
export function parseArgs(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
}

/** Dane komendy `mcp_save`. */
export function toInput(f: McpForm): McpServerInput {
  return {
    id: f.id,
    name: f.name.trim(),
    command: f.command.trim(),
    args: parseArgs(f.argsText),
    cwd: f.cwd.trim() || null,
    env: f.env
      .filter((e) => e.name.trim().length > 0)
      .map((e) => ({ name: e.name.trim(), value: e.value || null, secret: e.secret })),
    trust: f.trust,
    enabled: f.enabled,
    timeout_ms: Math.round(Math.max(1, f.timeoutSec) * 1000),
  };
}

/** Brakujące pola formularza (klucze i18n) — przycisk „Zapisz” nieaktywny, dopóki lista niepusta. */
export function formProblems(f: McpForm): string[] {
  const out: string[] = [];
  if (!f.name.trim()) out.push('mcp.form.needName');
  if (!f.command.trim()) out.push('mcp.form.needCommand');
  if (parseArgs(f.argsText).some((a) => a === FOLDER)) out.push('mcp.form.needFolder');
  // Wartość tajna: `null` przy edycji = zapisana wcześniej (zostaje); pusty tekst albo nowy serwer
  // bez wartości = brak.
  const secretMissing = f.env.some(
    (e) => e.secret && e.name.trim() && !e.value && (f.id === null || e.value === ''),
  );
  if (secretMissing) out.push('mcp.form.needSecret');
  return out;
}

/** Czy narzędzie może być wywołane przez agentkę. */
export const toolReady = (t: McpToolView) => t.state === 'approved' || t.state === 'auto_approved';

/** Czego brakuje do uruchomienia przepisu: program nieznaleziony w PATH. */
export function runtimeOf(command: string): McpRuntime | null {
  const c = command.trim().toLowerCase();
  if (c === 'npx' || c === 'node' || c === 'npm') return 'node';
  if (c === 'uvx' || c === 'uv') return 'uv';
  return null;
}
