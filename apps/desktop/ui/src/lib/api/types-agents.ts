// DTO agentek z narzędziami (Replay krok po kroku, intencje „uruchom w terminalu", katalog roboczy
// sesji) i trybu głosowego (stan potoku). Odpowiednik `crates/app-api/src/dto/agents.rs`.
import type { AgentId } from '@alfa/ui-kit';
import type { Iso8601, LocalizedText, Money } from './types';

/** Akcja, którą właściciel wykonuje sam (UI nic nie uruchamia). */
export type IntentKind = 'open_in_terminal' | 'confirm_delete_permanent';

export interface ToolIntent {
  readonly kind: IntentKind;
  readonly title: string;
  /** Polecenie do skopiowania (terminal) — nigdy wykonywane automatycznie. */
  readonly command: string | null;
  readonly cwd: string | null;
  readonly shell: string | null;
  readonly paths: readonly string[];
}

export type RunState =
  | 'running'
  | 'waiting_approval'
  | 'paused'
  | 'completed'
  | 'cancelled'
  | 'failed'
  | 'budget_exceeded'
  | 'loop_detected'
  | 'refused';

export type ReplayKind = 'plan' | 'think' | 'tool' | 'verify' | 'steer';

export type ReplayStatus =
  'running' | 'ok' | 'denied' | 'needs_confirmation' | 'failed' | 'cancelled' | 'waiting_approval';

/** Krok przebiegu: krok, narzędzie, wejście/wyjście w skrócie (zwykły tekst), status, czas. */
export interface ReplayStep {
  readonly id: string;
  readonly n: number;
  readonly kind: ReplayKind;
  readonly tool: string | null;
  readonly title: string;
  readonly input: string;
  readonly output: string;
  readonly status: ReplayStatus;
  /** Czas od startu przebiegu. */
  readonly at_ms: number;
  readonly duration_ms: number | null;
  /** Token „Cofnij krok" (dziennik cofania albo schowek). */
  readonly undo_token: string | null;
  readonly undone: boolean;
  /** Wynik niósł treść z zewnątrz (plik, polecenie, schowek). */
  readonly untrusted: boolean;
  readonly intent: ToolIntent | null;
  readonly approval_id: string | null;
}

export interface RunUsage {
  readonly steps: number;
  readonly tool_calls: number;
  readonly input_tokens: number;
  readonly output_tokens: number;
  readonly cost: Money;
  readonly elapsed_ms: number;
}

export interface RunBudgetView {
  readonly max_steps: number;
  readonly max_minutes: number;
  readonly max_cost: Money | null;
}

/** Przebieg agentki (nagłówek). */
export interface AgentRun {
  readonly id: string;
  readonly session_id: string;
  readonly turn_id: string | null;
  readonly agent: AgentId;
  readonly goal: string;
  readonly workdir: string | null;
  readonly state: RunState;
  readonly started_at: Iso8601;
  readonly finished_at: Iso8601 | null;
  readonly summary: string | null;
  readonly usage: RunUsage;
  readonly budget: RunBudgetView;
  /** Przebieg mostu CLI (`claude_code`, `codex`) — wynik niezweryfikowany przez Alfę. */
  readonly bridge?: string;
  /** Zadanie schedulera, w ramach którego trwa przebieg. */
  readonly task_id?: string;
  /** Przebieg-rodzic (podprzebieg: delegacja, Krytyczka, umiejętność). */
  readonly parent_id?: string;
  /** Etykieta podprzebiegu („Krytyczka", „delegacja: …"). */
  readonly label?: string;
}

export interface AgentRunDetail {
  readonly run: AgentRun;
  readonly steps: readonly ReplayStep[];
}

export type WorkdirChoice = 'dialog' | 'default' | 'none';

/** Katalog roboczy sesji = zakres narzędzi agentek; `null` — agentki bez narzędzi. */
export interface SessionWorkdir {
  readonly path: string | null;
  readonly default_path: string;
}

export type VoiceState = 'unavailable' | 'off' | 'active';
export type VoiceMode = 'toggle' | 'ptt';
export type VoiceSpeaker = 'nobody' | 'user' | 'agent';

export interface VoiceStatus {
  readonly state: VoiceState;
  /** Powód niedostępności (np. „pobierz modele w Ustawieniach → Głos"). */
  readonly reason: LocalizedText | null;
  readonly missing: readonly string[];
  readonly mode: VoiceMode;
  readonly muted: boolean;
  readonly agent: AgentId;
}
