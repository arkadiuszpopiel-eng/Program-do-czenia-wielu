// Typy DTO warstwy danych UI (IPC Tauri ↔ Svelte).
// Pola w snake_case — tak jak serializuje je serde po stronie Rust. Gdy kontrakty `*-contract`
// dostaną generator (tauri-specta / ts-rs, ADR 0013), ten plik zostanie zastąpiony typami
// generowanymi 1:1; do tego czasu jest jedynym miejscem definicji kształtu danych w UI.
import type { AgentId, RiskLevel } from '@alfa/ui-kit';
import type { ToolIntent } from './types-agents';

export type Locale = 'pl' | 'en';
/** Data i czas w RFC 3339 (UTC). */
export type Iso8601 = string;
/** Tekst zlokalizowany dostarczany przez backend (np. strony ustawień z manifestów modułów). */
export interface LocalizedText {
  readonly pl: string;
  readonly en: string;
}

/** Kwota w jednostkach drobnych (grosze / centy) — bez liczb zmiennoprzecinkowych w pieniądzach. */
export interface Money {
  readonly minor: number;
  readonly currency: 'PLN' | 'USD';
}

export type AutonomyLevel = 'L0' | 'L1' | 'L2' | 'L3' | 'L4';
export type ModelProfile = 'local' | 'hybrid' | 'cloud';

// ── Sesje ───────────────────────────────────────────────────────────────────────────────────────

export interface ProjectRef {
  readonly id: string;
  readonly name: string;
}

export interface SessionSummary {
  readonly id: string;
  readonly title: string;
  readonly project: ProjectRef | null;
  readonly pinned: boolean;
  readonly archived: boolean;
  /** Kropka aktywności: sesja pracuje w tle. */
  readonly working: boolean;
  readonly unread: boolean;
  readonly updated_at: Iso8601;
  readonly tags: readonly string[];
  readonly autonomy: AutonomyLevel;
  readonly profile: ModelProfile;
}

export type SessionTemplate = 'empty' | 'coding' | 'research' | 'voice' | 'admin';

/** Usunięcie cofalne przez 10 s (Kosz logiczny, potem crypto-shredding w backendzie). */
export interface UndoTicket {
  readonly token: string;
  readonly expires_at: Iso8601;
}

export interface SessionSearchHit {
  readonly session_id: string;
  readonly title: string;
  /** Fragment tekstu (zwykły tekst, nie HTML). */
  readonly snippet: string;
  readonly turn_id: string | null;
}

// ── Tury (append-only, drzewo gałęzi) ────────────────────────────────────────────────────────────

export type Author = 'user' | AgentId;
export type TurnStatus = 'queued' | 'streaming' | 'complete' | 'cancelled' | 'error';

/**
 * Blok treści wyrenderowany w Rust (pulldown-cmark + ammonia). UI wstawia `html_sanitized`
 * bez żadnego przetwarzania. Bloki zamknięte (`closed`) nie zmieniają się — nie są przerenderowywane.
 */
export interface RenderedBlock {
  readonly index: number;
  readonly kind: 'text' | 'code';
  readonly lang: string | null;
  readonly html_sanitized: string;
  readonly closed: boolean;
}

export type ToolIcon = 'file' | 'terminal' | 'search' | 'edit' | 'web' | 'memory';

export interface ToolStep {
  readonly id: string;
  readonly icon: ToolIcon;
  readonly label: string;
  readonly status: 'running' | 'done' | 'error';
  readonly duration_ms: number | null;
  /** Token dziennika cofania (akcje `fs.*`, snapshot powłoki, schowek) — przycisk „Cofnij". */
  readonly undo_token: string | null;
  /** Krok już cofnięty (po ponownym wczytaniu sesji). */
  readonly undone: boolean;
  /** Akcja do wykonania przez właściciela („uruchom w terminalu", trwałe usunięcie). */
  readonly intent: ToolIntent | null;
}

/** Karta „czeka na zatwierdzenie": UI tylko przenosi do okna Brokera (PLAN §8.2). */
export interface ApprovalPending {
  readonly id: string;
  readonly what: string;
  readonly why: string;
  readonly reversible: boolean;
  readonly risk: RiskLevel;
  readonly status: 'pending' | 'approved' | 'denied' | 'expired';
  /** Czy działa okno Brokera; bez niego (tryb deweloperski) prośba wygaśnie i agentka dostanie odmowę. */
  readonly broker_window: boolean;
  /** Kiedy prośba wygaśnie (limit czekania agentki). */
  readonly expires_at: Iso8601 | null;
}

export interface TurnUsage {
  readonly input_tokens: number;
  readonly output_tokens: number;
  readonly cost: Money;
  readonly latency_ms: number;
  readonly provider: string;
  readonly model: string;
}

export type TurnErrorCode =
  'offline' | 'rate_limited' | 'no_keys' | 'provider' | 'context_overflow' | 'budget_blocked';

export interface TurnError {
  readonly code: TurnErrorCode;
  readonly message: string;
  /** Kiedy odnowi się limit (429 / okno planu). */
  readonly retry_at: Iso8601 | null;
  readonly provider: string | null;
}

export interface Turn {
  readonly id: string;
  readonly session_id: string;
  readonly parent_id: string | null;
  readonly author: Author;
  /** Identyfikator roli agentki w chwili odpowiedzi (np. `conductor`) — etykieta z i18n. */
  readonly role_id: string | null;
  readonly created_at: Iso8601;
  status: TurnStatus;
  /** Surowy tekst (do kopiowania jako Markdown i dla `aria-live`). Nigdy nie renderowany jako HTML. */
  text: string;
  blocks: RenderedBlock[];
  thinking: { duration_ms: number; active: boolean } | null;
  tools: ToolStep[];
  approval: ApprovalPending | null;
  usage: TurnUsage | null;
  error: TurnError | null;
  /** Tura kontynuowana przez tę turę („kontynuuj" uciętą odpowiedź). */
  readonly continues: string | null;
  readonly addressed_to: AgentId | null;
  /** StopReason MaxTokens → akcja „Kontynuuj". */
  truncated: boolean;
  /** Odpowiedź głosowa przerwana: tekst, który usłyszał użytkownik (`null` = cała). */
  heard_prefix: string | null;
}

/** Adnotacje widoku — osobne rekordy dziennika, nie zmieniają tury. */
export interface TurnAnnotation {
  readonly rating: 'up' | 'down' | null;
  readonly hidden: boolean;
}

export interface TurnsSnapshot {
  readonly turns: readonly Turn[];
  readonly annotations: Readonly<Record<string, TurnAnnotation>>;
}

export interface SendOptions {
  readonly parent_id: string | null;
  readonly text: string;
  readonly addressed_to: AgentId | null;
  readonly profile: ModelProfile | null;
}

export interface SendResult {
  readonly user_turn_id: string;
  /** `null`, gdy wiadomość czeka w kolejce (offline). */
  readonly assistant_turn_id: string | null;
}

export type RememberScope = 'session' | 'project' | 'global' | 'agent';

// ── Agentki, aktywność, koszty ──────────────────────────────────────────────────────────────────

export type AgentStatus = 'idle' | 'speaking' | 'working' | 'waiting_approval';

export interface AgentState {
  readonly id: AgentId;
  readonly role_ids: readonly string[];
  readonly status: AgentStatus;
  /** Bieżąca czynność (tekst od backendu, np. „edytuję raport.docx"). */
  readonly activity: string | null;
}

export type CastTemplateId = 'standard' | 'solo' | 'coding' | 'research';

export interface ActivityInfo {
  readonly session_id: string;
  readonly agent: AgentId;
  readonly description: string;
  readonly step: number;
  readonly total_steps: number;
  readonly started_at: Iso8601;
}

export interface CostSummary {
  readonly session: Money;
  readonly day: Money;
  readonly month: Money;
  readonly limit: { readonly enabled: boolean; readonly monthly: Money };
  readonly context: {
    readonly used_tokens: number;
    readonly max_tokens: number;
    readonly compacted: boolean;
  };
  readonly fx: { readonly usd_pln: number; readonly date: string; readonly stale: boolean };
}

// ── Oś czasu ────────────────────────────────────────────────────────────────────────────────────

export type TimelineKind = 'model_call' | 'tool' | 'audit' | 'ui' | 'voice' | 'diagnostics';
export type EventLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error' | 'audit';

export interface TimelineEvent {
  readonly id: string;
  readonly ts: Iso8601;
  readonly session_id: string;
  readonly kind: TimelineKind;
  readonly level: EventLevel;
  readonly agent: AgentId | null;
  readonly title: string;
  readonly detail: string | null;
  readonly cost: Money | null;
  readonly latency_ms: number | null;
  readonly turn_id: string | null;
}

export interface TimelineFilter {
  readonly kinds: readonly TimelineKind[];
  readonly min_level: EventLevel;
}

// ── Pliki / artefakty ───────────────────────────────────────────────────────────────────────────

export interface ArtifactInfo {
  readonly id: string;
  readonly session_id: string;
  readonly name: string;
  readonly path: string;
  readonly size_bytes: number;
  readonly mime: string;
  readonly created_at: Iso8601;
  readonly agent: AgentId | null;
  readonly versions: number;
}

/** Podgląd: tekst jako zwykły tekst (nigdy HTML); obraz przez protokół zasobów (ścieżka, nie bajty). */
export type ArtifactPreview =
  | { readonly kind: 'text'; readonly text: string; readonly truncated: boolean }
  | { readonly kind: 'image'; readonly src: string }
  | { readonly kind: 'none' };

export type ArtifactAction = 'open' | 'reveal' | 'copy' | 'save_as';

export type * from './types-agents';
export type * from './types-hub';
export type * from './types-system';
