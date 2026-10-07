// DTO Inspektora pamięci (F7, makieta 9; odpowiedniki `app-api/src/dto/memory.rs`): zakresy,
// wpisy z proweniencją, „dlaczego to pamiętam", edycja = nowa wersja, zapomnienie z podglądem
// kaskady, dziennik zmian z cofaniem, porządkowanie (Strażniczka pamięci).
// Identyfikator wpisu: `"<klucz zakresu>#<id>"` (np. `session:s-1#mem-1`, `global#mem-2`).
import type { Iso8601 } from './types';

export type MemoryScopeKind = 'session' | 'project' | 'agent' | 'global';

/** Zakres (`id` = sesja / projekt / agentka; `null` dla globalnej). */
export interface MemoryScopeRef {
  readonly kind: MemoryScopeKind;
  readonly id: string | null;
}

export type MemoryLayer = 'working' | 'episodic' | 'semantic' | 'procedural';
export type MemoryState = 'active' | 'pending' | 'superseded' | 'expired';
export type MemorySourceKind = 'user' | 'agent' | 'untrusted' | 'import';

export interface MemoryScopeInfo {
  readonly key: string;
  readonly scope: MemoryScopeRef;
  readonly label: string;
  readonly entries: number;
  readonly active: number;
  readonly pending: number;
  /** Dokument w paczce `.alfa` (`null` — sesja prywatna, poza eksportem). */
  readonly document: string | null;
}

/** Zapytanie Inspektora (filtry łączone AND; puste listy = bez filtra). */
export interface MemoryQuery {
  readonly scopes: readonly string[];
  readonly text: string | null;
  readonly layers: readonly MemoryLayer[];
  readonly states: readonly MemoryState[];
  readonly trusted: boolean | null;
  readonly pinned: boolean | null;
  readonly offset: number;
  readonly limit: number;
}

export interface MemoryItem {
  readonly id: string;
  readonly scope: MemoryScopeRef;
  readonly scope_key: string;
  readonly layer: MemoryLayer;
  readonly state: MemoryState;
  readonly text: string;
  readonly subject: string | null;
  readonly entities: readonly string[];
  readonly source: MemorySourceKind;
  /** Agentka (pochodzenie `agent`) albo źródło (treść niezaufana, import). */
  readonly source_detail: string | null;
  readonly trusted: boolean;
  readonly confidence: number;
  readonly pinned: boolean;
  readonly version: number;
  readonly created_at: Iso8601;
  readonly expires_at: Iso8601 | null;
  readonly session_id: string | null;
  readonly turn: number | null;
  /** `extracted`, `summary`, `skill`, `promoted`, `edited`, `imported`. */
  readonly derivation: string | null;
  readonly score: number | null;
}

export interface MemoryPage {
  readonly items: readonly MemoryItem[];
  readonly total: number;
}

export interface MemorySourceLink {
  readonly id: string;
  readonly exists: boolean;
  readonly state: MemoryState | null;
}

export interface MemoryJournalEntry {
  readonly id: string;
  readonly scope_key: string;
  /** Przebieg porządkowania (`null` = zmiana użytkownika). */
  readonly run: string | null;
  readonly at: Iso8601;
  /** `create`, `supersede`, `merge`, `expire`, `mark_consolidated`, `conflict`, `edit`, `promote`. */
  readonly kind: string;
  readonly note: string;
  readonly entries: readonly string[];
  readonly undone: boolean;
  /** Czy da się cofnąć (wygaszenia są nieodwracalne). */
  readonly undoable: boolean;
}

/** „Dlaczego to pamiętam". */
export interface MemoryExplanation {
  readonly item: MemoryItem;
  readonly reasons: readonly string[];
  readonly sources: readonly MemorySourceLink[];
  /** Historia wersji od najstarszej (łącznie z tym wpisem). */
  readonly versions: readonly MemoryItem[];
  readonly merged: readonly string[];
  readonly derived: readonly string[];
  readonly journal: readonly MemoryJournalEntry[];
}

/** Edycja = nowa wersja (pola `null` bez zmian; pusty `subject` usuwa temat). */
export interface MemoryEdit {
  readonly text: string | null;
  readonly subject: string | null;
  readonly confidence: number | null;
}

export type MemoryForgetTarget =
  | { readonly target: 'entry'; readonly id: string }
  | { readonly target: 'scope'; readonly scope: string };

export interface MemoryCascadeItem {
  readonly id: string;
  readonly scope_key: string;
  /** Początek treści (do 120 znaków). */
  readonly text: string;
  /** `target`, `version` albo `derived`. */
  readonly reason: string;
}

export interface MemoryForgetPreview {
  readonly target: MemoryForgetTarget;
  readonly remove: readonly MemoryCascadeItem[];
  /** Wpisy, które wrócą do stanu aktywnego (były zastąpione przez usuwane). */
  readonly revive: readonly string[];
  /** Zakres zostanie usunięty w całości (crypto-shredding bazy). */
  readonly shred: boolean;
}

export interface MemoryForgetReport {
  readonly removed: number;
  readonly derived: number;
  readonly versions: number;
  readonly revived: number;
  readonly fts_rows: number;
  readonly vectors: number;
  readonly journal_records: number;
  readonly shredded: readonly string[];
  readonly stale_exports: readonly string[];
}

export interface MemoryUndoResult {
  readonly removed: number;
  readonly restored: number;
  readonly skipped: number;
}

/** Raport porządkowania pamięci (Strażniczka). */
export interface ConsolidationReport {
  readonly run: string;
  readonly manual: boolean;
  readonly started_at: Iso8601;
  /** Dlaczego nie wystartował (bateria, tryb gry, okno, bezczynność, wyłączona…). */
  readonly skipped: string | null;
  readonly interrupted: string | null;
  readonly scopes: number;
  readonly created: number;
  readonly merged: number;
  readonly resolved: number;
  readonly expired: number;
  readonly conflicts: number;
  readonly proposals: number;
  readonly llm_calls: number;
  readonly budget_denied: boolean;
  readonly errors: readonly string[];
}

export interface MemoryStatus {
  readonly consolidation_enabled: boolean;
  /** Licznik bezczynności (bez niego harmonogram nocny nie startuje; ręcznie — tak). */
  readonly idle_available: boolean;
  /** Model lokalny dla porządkowania (bez niego — tylko reguły deterministyczne). */
  readonly model_available: boolean;
  readonly window: string;
  readonly pending: number;
  readonly last: ConsolidationReport | null;
}
