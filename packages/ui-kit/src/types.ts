import type { AgentId, RiskLevel } from './tokens';

/** Stan mikrofonu (PLAN.md §14.9): zawsze ikona + tekst, nigdy sam kolor. */
export type MicState =
  'off' | 'listening' | 'hearing' | 'processing' | 'speaking' | 'muted' | 'dnd';

export type ToastKind = 'info' | 'success' | 'warning' | 'error';

export interface ToolStep {
  readonly id: string;
  readonly icon?: 'file' | 'terminal' | 'search' | 'edit';
  readonly label: string;
  readonly durationMs?: number;
  readonly undoable?: boolean;
}

export interface ApprovalRequest {
  readonly what: string;
  readonly why: string;
  readonly reversible: boolean;
  readonly risk: RiskLevel;
}

export interface ChatMessage {
  readonly id: string;
  readonly author: 'user' | AgentId;
  readonly role?: string;
  readonly text: string;
  readonly time: string;
  readonly steps?: readonly ToolStep[];
  readonly approval?: ApprovalRequest;
  readonly variants?: { readonly index: number; readonly total: number };
  readonly streaming?: boolean;
}

export interface SessionItem {
  readonly id: string;
  readonly title: string;
  readonly project?: string;
  readonly active?: boolean;
  readonly working?: boolean;
  readonly unread?: boolean;
}

export interface ActivityInfo {
  readonly agent: AgentId;
  readonly description: string;
  readonly step: number;
  readonly totalSteps: number;
  readonly elapsedSeconds: number;
}

export interface CommandItem {
  readonly id: string;
  readonly label: string;
  readonly group: string;
  readonly shortcut?: string;
  readonly keywords?: readonly string[];
  readonly onSelect?: () => void;
}

/** Fragment napisów na żywo: słowa już wypowiedziane (prefix) są podświetlane. */
export interface CaptionLine {
  readonly speaker: 'user' | AgentId;
  readonly text: string;
  /** Liczba znaków już wypowiedzianych / zatwierdzonych. */
  readonly spokenChars: number;
  readonly partial?: boolean;
  /** Indeks znaku, w którym użytkownik przerwał agentkę. */
  readonly interruptedAt?: number;
}
