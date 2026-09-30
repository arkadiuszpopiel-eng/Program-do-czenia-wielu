// DTO: ustawienia (drzewo §15), stan systemu (§14.4), start aplikacji, układ okna i zdarzenia.
import type { AgentId, MicState } from '@alfa/ui-kit';
import type {
  ActivityInfo,
  AgentState,
  ApprovalPending,
  CostSummary,
  Iso8601,
  LocalizedText,
  Locale,
  ModelProfile,
  RenderedBlock,
  SessionSummary,
  TimelineEvent,
  ToolStep,
  Turn,
  TurnError,
  TurnStatus,
  TurnUsage,
} from './types';
import type { Account } from './types-hub';

// ── Ustawienia ──────────────────────────────────────────────────────────────────────────────────

export type SettingValue = boolean | number | string;
export type SettingScope = 'global' | 'session' | 'agent' | 'machine';

export type SettingControl =
  | { readonly kind: 'toggle' }
  | {
      readonly kind: 'select';
      readonly options: readonly { readonly value: string; readonly label: LocalizedText }[];
    }
  | {
      readonly kind: 'number';
      readonly min: number;
      readonly max: number;
      readonly step: number;
      readonly unit: string | null;
    }
  | { readonly kind: 'text' };

export interface SettingDef {
  readonly key: string;
  readonly label: LocalizedText;
  readonly description: LocalizedText;
  readonly control: SettingControl;
  readonly default: SettingValue;
  readonly scope: SettingScope;
}

/** Strona o specjalnym widoku w UI (reszta renderowana generycznie z `settings`). */
export type SettingsCustomPage =
  'providers' | 'costs' | 'shortcuts' | 'transfer' | 'permissions' | 'devices';

export interface SettingsPageDef {
  readonly id: string;
  readonly label: LocalizedText;
  /** Fala, od której strona działa; późniejsze pokazują pusty stan „dostępne w kolejnej fali". */
  readonly wave: number;
  readonly custom: SettingsCustomPage | null;
  readonly settings: readonly SettingDef[];
  /** Co pojawi się na stronie w przyszłej fali (dla pustego stanu). */
  readonly upcoming: readonly LocalizedText[];
}

// ── Stan systemu (PLAN §14.4) ───────────────────────────────────────────────────────────────────

export interface SystemStatus {
  readonly online: boolean;
  /** Wiadomości czekające w kolejce (offline). */
  readonly queued_messages: number;
  readonly rate_limit: { readonly provider: string; readonly resets_at: Iso8601 } | null;
  readonly keys_configured: boolean;
  readonly profile: ModelProfile;
  readonly mic: 'ok' | 'denied' | 'missing';
  readonly disk: { readonly free_bytes: number; readonly low: boolean };
}

// ── Start aplikacji i układ okna ────────────────────────────────────────────────────────────────

export type PanelId = 'agents' | 'timeline' | 'files' | 'memory' | 'screen' | 'voice';

/** Które panele są otwarte — per sesja (PLAN §14.2). */
export interface SessionPanels {
  readonly left_open: boolean;
  readonly right_open: boolean;
  readonly right_tab: PanelId;
}

/** Szerokości paneli — per maszyna (nakładka `config/machine/<id>.toml`). */
export interface LayoutPrefs {
  readonly left_width: number;
  readonly right_width: number;
  readonly left_collapsed: boolean;
  readonly sessions: Readonly<Record<string, SessionPanels>>;
}

export interface AppBootstrap {
  readonly app_version: string;
  readonly locale: Locale;
  readonly onboarding_done: boolean;
  readonly machine_name: string;
  readonly settings: Readonly<Record<string, SettingValue>>;
  readonly layout: LayoutPrefs | null;
  readonly active_session_id: string | null;
  readonly shortcut_overrides: Readonly<Record<string, string>>;
}

// ── Szybkie pytanie / pigułka głosowa ───────────────────────────────────────────────────────────

export interface QuickAskResult {
  readonly session_id: string;
  readonly user_turn_id: string;
  readonly assistant_turn_id: string | null;
}

export interface VoicePillState {
  readonly agent: AgentId;
  readonly mic: MicState;
  readonly level: number;
}

// ── Zdarzenia (jeden kanał, batch co klatkę) ────────────────────────────────────────────────────

/** Strumień odpowiedzi — nazwy zgodne z `ChatEvent` (providers-api) i PLAN §13. */
export type ChatStreamEvent =
  | { readonly type: 'TurnAppended'; readonly session_id: string; readonly turn: Turn }
  | {
      /** Zmiana stanu tury bez zmiany treści (np. wiadomość z kolejki offline została wysłana). */
      readonly type: 'TurnStatus';
      readonly session_id: string;
      readonly turn_id: string;
      readonly status: TurnStatus;
    }
  | {
      readonly type: 'TextDelta';
      readonly session_id: string;
      readonly turn_id: string;
      readonly text: string;
      /** Tylko zmienione bloki: otwarty (ostatni) i nowo zamknięte. */
      readonly blocks: readonly RenderedBlock[];
    }
  | {
      readonly type: 'ThinkingDelta';
      readonly session_id: string;
      readonly turn_id: string;
      readonly elapsed_ms: number;
      readonly done: boolean;
    }
  | {
      readonly type: 'ToolCall';
      readonly session_id: string;
      readonly turn_id: string;
      readonly step: ToolStep;
    }
  | {
      readonly type: 'ApprovalPending';
      readonly session_id: string;
      readonly turn_id: string;
      readonly approval: ApprovalPending;
    }
  | {
      readonly type: 'Usage';
      readonly session_id: string;
      readonly turn_id: string;
      readonly usage: TurnUsage;
    }
  | {
      readonly type: 'Stop';
      readonly session_id: string;
      readonly turn_id: string;
      readonly reason: 'end' | 'refusal' | 'tool_use' | 'max_tokens' | 'cancelled';
    }
  | {
      readonly type: 'Error';
      readonly session_id: string;
      readonly turn_id: string;
      readonly error: TurnError;
    };

export type AlfaEvent =
  | ChatStreamEvent
  | { readonly type: 'SessionUpdated'; readonly session: SessionSummary }
  | { readonly type: 'SessionRemoved'; readonly session_id: string }
  | {
      readonly type: 'AgentsChanged';
      readonly session_id: string;
      readonly agents: readonly AgentState[];
    }
  | {
      readonly type: 'ActivityChanged';
      readonly session_id: string;
      readonly activity: ActivityInfo | null;
    }
  | { readonly type: 'CostsChanged'; readonly session_id: string; readonly costs: CostSummary }
  | { readonly type: 'SystemStatusChanged'; readonly status: SystemStatus }
  | { readonly type: 'TimelineAppended'; readonly event: TimelineEvent }
  | { readonly type: 'AccountChanged'; readonly account: Account }
  | { readonly type: 'MicLevel'; readonly level: number }
  | { readonly type: 'VoicePill'; readonly state: VoicePillState }
  | {
      readonly type: 'Toast';
      readonly kind: 'info' | 'success' | 'warning' | 'error';
      readonly message: LocalizedText;
    };

export type AlfaEventType = AlfaEvent['type'];
export type Unsubscribe = () => void;
