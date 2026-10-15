// Atrapa: Tablica — przypięcia w pamięci (do 40, ta sama tura raz), treść HTML jak z rdzenia.
import type { BoardApi } from '../client-board';
import type { BoardItem } from '../types-board';
import type { FakeCore } from './core';

const MAX = 40;

const html = (text: string): string =>
  `<p>${text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')}</p>`;

export class FakeBoard {
  private items: BoardItem[] = [
    {
      id: 'pin-1',
      session_id: 's-q3',
      session_title: 'Raport Q3',
      turn_id: 'q3',
      agent: 'beta',
      text: 'Przychody wzrosły o 12 % rok do roku.',
      html_sanitized: html('Przychody wzrosły o 12 % rok do roku.'),
      pinned_at: '2026-10-15T09:30:00Z',
    },
  ];

  constructor(private readonly core: FakeCore) {}

  private pin(sessionId: string, turnId: string): readonly BoardItem[] {
    if (this.items.some((i) => i.session_id === sessionId && i.turn_id === turnId)) {
      return this.items;
    }
    if (this.items.length >= MAX) {
      throw new Error(`Tablica jest pełna (${MAX} przypięć) — odepnij coś, żeby dodać nowe.`);
    }
    const turn = this.core.turnsOf(sessionId).find((t) => t.id === turnId);
    if (!turn || turn.author === 'user') {
      throw new Error('Przypiąć można tylko odpowiedź agentki.');
    }
    const session = this.core.session(sessionId);
    const item: BoardItem = {
      id: `pin-${this.items.length + 1}-${turnId}`,
      session_id: sessionId,
      session_title: session?.title ?? sessionId,
      turn_id: turnId,
      agent: turn.author,
      text: turn.text,
      html_sanitized: html(turn.text),
      pinned_at: new Date(this.core.scheduler.now()).toISOString(),
    };
    this.items = [item, ...this.items];
    return this.items;
  }

  api(): BoardApi {
    return {
      list: () => this.core.reply(this.items),
      pin: async (sessionId, turnId) => this.core.reply(this.pin(sessionId, turnId)),
      unpin: async (id) => {
        this.items = this.items.filter((i) => i.id !== id);
        return this.core.reply(this.items);
      },
    };
  }
}
