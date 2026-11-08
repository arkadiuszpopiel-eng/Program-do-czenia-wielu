// Stan okna Szybkiego pytania bez DOM: pytanie → sesja z wyniku `quick_ask` → odpowiedź ze zdarzeń.
// Paczka zdarzeń rdzenia i odpowiedź IPC to dwa niezależne kanały: tura odpowiedzi może przyjść,
// zanim okno pozna ID sesji. Dlatego zdarzenia rozmowy z czasu żądania są buforowane i po wyniku
// odtwarzane — tylko te z sesji, którą zwrócił `quick_ask`.
import type { AlfaClient } from '../lib/api/client';
import type { Turn } from '../lib/api/types';
import type { AlfaEvent, ChatStreamEvent } from '../lib/api/types-system';
import { applyTurnEvent, isChatEvent } from '../lib/logic/apply-event';

/** Górna granica bufora (żądanie trwa chwilę; chroni pamięć, gdy rdzeń nie odpowiada). */
const MAX_PENDING = 5000;

export class QuickSession {
  sessionId = $state<string | null>(null);
  answer = $state<Turn | null>(null);
  queued = $state(false);
  /** Zdarzenia rozmowy z czasu trwającego `quick_ask` (`null` — brak żądania). */
  private pending: ChatStreamEvent[] | null = null;

  constructor(private readonly client: AlfaClient) {}

  /** Paczka zdarzeń z kanału `alfa://events` (najwyżej raz na klatkę). */
  apply(batch: readonly AlfaEvent[]): void {
    for (const event of batch) {
      if (!isChatEvent(event)) continue;
      if (this.pending) {
        this.pending.push(event);
        if (this.pending.length > MAX_PENDING) this.pending.shift();
      } else this.applyChat(event);
    }
  }

  /** Wysyła pytanie; odrzucenie przekazuje dalej (okno przywraca treść pola). */
  async ask(text: string): Promise<void> {
    this.answer = null;
    const pending: ChatStreamEvent[] = [];
    this.pending = pending;
    try {
      const result = await this.client.quick.ask(text);
      this.sessionId = result.session_id;
      this.queued = result.assistant_turn_id === null;
    } finally {
      // Nowsze pytanie mogło już założyć własny bufor — wtedy zbiera on dalsze zdarzenia.
      if (this.pending === pending) this.pending = null;
      for (const event of pending) this.applyChat(event);
    }
  }

  private applyChat(event: ChatStreamEvent): void {
    if (event.session_id !== this.sessionId) return;
    if (event.type === 'TurnAppended') {
      if (event.turn.author !== 'user') this.answer = event.turn;
    } else if (this.answer && event.turn_id === this.answer.id) applyTurnEvent(this.answer, event);
  }
}
