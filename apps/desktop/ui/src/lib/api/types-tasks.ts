// DTO panelu Zadania (DAG, stan, postęp, sterowanie), Wyzwalaczy, Reguł Marszałka i kart
// zgodności mostów CLI — odpowiedniki `app-api/src/dto/{tasks,bridges}.rs`.
import type { Iso8601, Money } from './types';
import type { ComplianceStatus } from './types-hub';

export type TaskStateKind = 'pending' | 'ready' | 'running' | 'retry_wait' | 'paused' | 'done';
export type TaskResultKind =
  | 'succeeded'
  | 'failed'
  | 'cancelled'
  | 'skipped'
  | 'expired'
  | 'budget_exceeded'
  | 'budget_blocked';
export type TaskClassKind = 'user' | 'agent' | 'background';
/** Pochodzenie (mosty CLI tylko z `user` albo `schedule` ze zgodą). */
export type TaskOriginKind = 'user' | 'agent' | 'trigger' | 'schedule' | 'improver' | 'system';

export interface TaskDep {
  readonly task_id: string;
  /** `succeeded`, `failed`, `finished`, `output_equals`. */
  readonly condition: string;
}

export interface TaskInfo {
  readonly id: string;
  readonly title: string;
  readonly parent_id: string | null;
  readonly deps: readonly TaskDep[];
  /** `alfa`, `role:coder`, `any`, `system:<usługa>`. */
  readonly assignee: string;
  readonly agent: string | null;
  readonly class: TaskClassKind;
  readonly origin: TaskOriginKind;
  readonly origin_detail: string | null;
  /** `agent`, `bridge:claude_code`, `bridge:codex`, `service:<usługa>`. */
  readonly executor: string;
  readonly state: TaskStateKind;
  readonly result: TaskResultKind | null;
  readonly blocked: string | null;
  readonly error: string | null;
  readonly summary: string | null;
  readonly attempt: number;
  readonly max_attempts: number;
  readonly steps: number;
  readonly max_steps: number;
  readonly cost: Money;
  readonly session_id: string | null;
  readonly tainted: boolean;
  readonly submitted_at: Iso8601;
  readonly deadline_at: Iso8601;
}

/** Nowe zadanie od użytkownika (`after` = poprzedniczki w DAG, `parent_id` = delegacja). */
export interface NewTaskInput {
  readonly session_id: string | null;
  readonly title: string;
  readonly goal: string;
  readonly agent: string | null;
  readonly after: readonly string[];
  readonly parent_id: string | null;
}

export type TriggerKindView =
  | { readonly kind: 'cron'; readonly expr: string }
  | { readonly kind: 'once'; readonly at: Iso8601 }
  | { readonly kind: 'interval'; readonly every_minutes: number }
  | { readonly kind: 'file_in_dir'; readonly dir: string; readonly pattern: string | null }
  | { readonly kind: 'new_message'; readonly session_id: string | null }
  | {
      readonly kind: 'task_finished';
      readonly task_prefix: string | null;
      /** `succeeded`, `failed`, `any`. */
      readonly outcome: string;
    }
  | { readonly kind: 'manual' };

export interface TriggerInfo {
  readonly id: string;
  readonly name: string;
  readonly kind: TriggerKindView;
  readonly enabled: boolean;
  /** `user`, `agent:<persona>`, `system:<usługa>`. */
  readonly owner: string;
  readonly title: string;
  readonly goal: string;
  readonly agent: string | null;
  /** Most CLI — tylko harmonogram użytkownika z jawną zgodą na karcie mostu. */
  readonly bridge: string | null;
  readonly tz: string;
  readonly next_fire_at: Iso8601 | null;
  readonly last_fire_at: Iso8601 | null;
  readonly fired: number;
  readonly suppressed: number;
  readonly deferred_until: Iso8601 | null;
  readonly respect_dnd: boolean;
  /** Obserwacja katalogów niedostępna na tej platformie (tylko „Uruchom teraz"). */
  readonly watch_unavailable: boolean;
}

export interface TriggerDraft {
  readonly name: string;
  readonly kind: TriggerKindView;
  readonly title: string;
  readonly goal: string;
  readonly agent: string | null;
  readonly bridge: string | null;
  readonly respect_dnd: boolean;
}

export interface TriggerRunInfo {
  readonly at: Iso8601;
  readonly trigger_id: string;
  readonly cause: string;
  /** `submitted`, `suppressed`, `deferred`, `failed`. */
  readonly outcome: string;
  readonly task_id: string | null;
  readonly detail: string | null;
}

/** Najbliższe uruchomienia wyrażenia cron (Europe/Warsaw, z DST). */
export interface CronPreview {
  readonly valid: boolean;
  readonly error: string | null;
  readonly next: readonly Iso8601[];
}

export interface MarshalRuleInfo {
  readonly id: string;
  readonly description: string;
  readonly when: readonly string[];
  readonly effects: readonly string[];
  readonly rule: unknown;
}

export interface MarshalRejected {
  readonly draft: unknown;
  readonly errors: readonly string[];
}

export interface MarshalProposalInfo {
  readonly id: number;
  readonly text: string;
  readonly rules: readonly MarshalRuleInfo[];
  readonly rejected: readonly MarshalRejected[];
  readonly conflicts: readonly string[];
  /** `pending`, `approved`, `rejected`. */
  readonly status: string;
  readonly created_at: Iso8601;
}

export interface MarshalState {
  readonly rules: readonly MarshalRuleInfo[];
  readonly proposals: readonly MarshalProposalInfo[];
  /** Polityka obowiązująca (sufit ∩ reguły) w punktach. */
  readonly effective: readonly string[];
  /** Tłumacz poleceń (model) dostępny — bez niego tylko reguły z edytora. */
  readonly translator: boolean;
}

export interface MarshalReport {
  readonly day: string;
  readonly submitted: number;
  readonly succeeded: number;
  readonly failed: number;
  readonly escalations: number;
  readonly text: string;
}

// ── Mosty CLI: karty zgodności ──────────────────────────────────────────────────────────────────

export interface BridgeSource {
  readonly url: string;
  readonly quote: string;
}

export interface BridgeCard {
  readonly route_id: string;
  /** `claude_code`, `codex`; `null` — trasa tylko w rejestrze (Alfa jej nie obsługuje). */
  readonly bridge: string | null;
  readonly name: string;
  readonly provider: string;
  /** `cli-p`, `sdk`, `api`. */
  readonly mode: string;
  readonly program: string | null;
  readonly detected: boolean;
  readonly path: string | null;
  readonly version: string | null;
  readonly pinned: readonly string[];
  readonly registry_pin: string | null;
  /** Wykryta wersja jest przypięta (inaczej most odmówi startu). */
  readonly version_ok: boolean;
  readonly status: ComplianceStatus;
  /** Wpis rejestru nieświeży — trasa zdegradowana do szarej. */
  readonly stale: boolean;
  readonly verified_at: string | null;
  readonly sources: readonly BridgeSource[];
  readonly allowed: readonly string[];
  readonly forbidden: readonly string[];
  readonly enabled: boolean;
  readonly can_enable: boolean;
  /** Zgoda na uruchomienia z harmonogramu: limit na dobę (0 = brak zgody). */
  readonly schedule_per_day: number;
  /** Polecenie logowania do wpisania przez użytkownika. */
  readonly login_command: string | null;
}

/** „Zaloguj w terminalu": polecenie do skopiowania (Alfa go nie wykonuje). */
export interface BridgeLogin {
  readonly command: string;
  readonly cwd: string;
  readonly opened: boolean;
}
