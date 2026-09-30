// Zastosowanie zdarzenia strumienia do tury (wspólne dla okna głównego i Szybkiego pytania).
// Mutuje tylko pola „żywe" tury (status, tekst, bloki, kroki…) — treść zamkniętych bloków
// zastępowana jest wyłącznie gotowym HTML-em z rdzenia.
import type { Turn } from '../api/types';
import type { ChatStreamEvent } from '../api/types-system';

export type TurnEvent = Exclude<ChatStreamEvent, { type: 'TurnAppended' }>;

/** Zwraca dopisany tekst (dla aria-live) albo `null`; `stopped` — koniec strumienia tej tury. */
export function applyTurnEvent(
  turn: Turn,
  event: TurnEvent,
): { text: string | null; stopped: boolean } {
  switch (event.type) {
    case 'TurnStatus':
      turn.status = event.status;
      return { text: null, stopped: false };
    case 'TextDelta':
      if (turn.status !== 'streaming') turn.status = 'streaming';
      if (event.text) turn.text += event.text;
      for (const block of event.blocks) turn.blocks[block.index] = block;
      return { text: event.text || null, stopped: false };
    case 'ThinkingDelta':
      turn.thinking = { duration_ms: event.elapsed_ms, active: !event.done };
      return { text: null, stopped: false };
    case 'ToolCall': {
      const index = turn.tools.findIndex((s) => s.id === event.step.id);
      if (index >= 0) turn.tools[index] = event.step;
      else turn.tools.push(event.step);
      return { text: null, stopped: false };
    }
    case 'ApprovalPending':
      turn.approval = event.approval;
      return { text: null, stopped: false };
    case 'Usage':
      turn.usage = event.usage;
      return { text: null, stopped: false };
    case 'Stop':
      turn.status = event.reason === 'cancelled' ? 'cancelled' : 'complete';
      turn.truncated = event.reason === 'max_tokens';
      if (turn.thinking?.active) turn.thinking = { ...turn.thinking, active: false };
      return { text: null, stopped: true };
    case 'Error':
      turn.status = 'error';
      turn.error = event.error;
      return { text: null, stopped: true };
  }
}

const CHAT_TYPES = new Set([
  'TurnAppended',
  'TurnStatus',
  'TextDelta',
  'ThinkingDelta',
  'ToolCall',
  'ApprovalPending',
  'Usage',
  'Stop',
  'Error',
]);

export function isChatEvent(event: { type: string }): event is ChatStreamEvent {
  return CHAT_TYPES.has(event.type);
}
