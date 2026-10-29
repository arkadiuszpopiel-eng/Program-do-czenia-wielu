// DTO: ustawienia (drzewo §15), stan systemu (§14.4), start aplikacji, układ okna i zdarzenia.
import type {
  ApprovalPending,
  RenderedBlock,
  ToolStep,
  Turn,
  TurnError,
  TurnStatus,
  TurnUsage,
} from './types';
import type { AlfaEvent } from './generated/ipc';
export type {
  AlfaEvent,
  AppBootstrap,
  EventFrame,
  EventsResume,
  LayoutPrefs,
  PanelId,
  QuickAskResult,
  SessionPanels,
  SettingControl,
  SettingDef,
  SettingScope,
  SettingValue,
  SettingsCustomPage,
  SettingsPageDef,
  SystemStatus,
  VoicePillState,
} from './generated/ipc';

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
      /** Długość tekstu tury przed tą deltą (jak `String.length`): pozwala pominąć część już znaną. */
      readonly offset: number;
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

export type AlfaEventType = AlfaEvent['type'];
export type Unsubscribe = () => void;
