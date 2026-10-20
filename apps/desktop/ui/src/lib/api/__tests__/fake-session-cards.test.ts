import { describe, expect, it } from 'vitest';
import { FakeAlfaClient, VirtualScheduler } from '../fake/fake-client';

function client() {
  return new FakeAlfaClient({ scheduler: new VirtualScheduler() });
}

describe('atrapa: karty rozmów (sessions.cards)', () => {
  it('karta „Raport Q3”: agentki, które odpowiadały, ostatnia wiadomość, model i koszt', async () => {
    const [card] = await client().sessions.cards(['s-q3']);
    expect(card?.session_id).toBe('s-q3');
    expect(card?.agents).toEqual(['gama', 'delta']);
    expect(card?.snippet_author).toBe('delta');
    expect(card?.snippet).toMatch(/^Utworzyłam szkic raport-Q3\.docx/);
    expect(card?.model).toBe('claude-opus-5-5');
    expect(card?.cost).toEqual({ minor: 108, currency: 'PLN' });
  });

  it('czysty tekst bez Markdownu; nieznane i powtórzone rozmowy pominięte', async () => {
    const cards = await client().sessions.cards(['s-q3', 'nie-ma', 's-q3', 's-shop']);
    expect(cards.map((c) => c.session_id)).toEqual(['s-q3', 's-shop']);
    for (const card of cards) {
      expect(card.snippet ?? '').not.toMatch(/[`*#<>]/);
      expect(card.snippet ?? '').not.toContain('\n');
    }
  });

  it('nowa rozmowa bez wiadomości: obsada domyślna, bez fragmentu i kosztu', async () => {
    const fake = client();
    const created = await fake.sessions.create('empty');
    const [card] = await fake.sessions.cards([created.id]);
    expect(card?.snippet).toBeNull();
    expect(card?.cost).toBeNull();
    expect(card?.model).toBeNull();
    expect(card?.agents).toEqual(['alfa']);
  });
});
