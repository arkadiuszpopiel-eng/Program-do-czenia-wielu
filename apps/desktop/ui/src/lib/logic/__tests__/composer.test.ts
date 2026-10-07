import { describe, expect, it } from 'vitest';
import { SentHistory, addressedAgent, applyCompletion, findTrigger } from '../composer';

describe('historia wysłanych (Ctrl+↑/↓)', () => {
  it('przegląda wstecz i wraca do szkicu', () => {
    const h = new SentHistory();
    h.push('pierwsza');
    h.push('druga');
    expect(h.prev('szkic')).toBe('druga');
    expect(h.prev('druga')).toBe('pierwsza');
    expect(h.prev('pierwsza')).toBe('pierwsza');
    expect(h.next()).toBe('druga');
    expect(h.next()).toBe('szkic');
    expect(h.next()).toBeNull();
  });

  it('pomija puste i powtórzenia, ma limit', () => {
    const h = new SentHistory(2);
    h.push('  ');
    h.push('a');
    h.push('a');
    h.push('b');
    h.push('c');
    expect(h.size).toBe(2);
    expect(new SentHistory().prev('x')).toBeNull();
  });
});

describe('@agentka i /komendy', () => {
  it('wykrywa token przy kursorze', () => {
    expect(findTrigger('Hej @de', 7)).toEqual({ kind: '@', query: 'de', start: 4, end: 7 });
    expect(findTrigger('/ob', 3)).toEqual({ kind: '/', query: 'ob', start: 0, end: 3 });
    expect(findTrigger('tekst /ob', 9)).toBeNull();
    expect(findTrigger('mail@domena', 11)).toBeNull();
  });

  it('wstawia podpowiedź', () => {
    const trigger = findTrigger('Hej @de zrób to', 7);
    if (!trigger) throw new Error('brak');
    expect(applyCompletion('Hej @de zrób to', trigger, 'Delta')).toEqual({
      text: 'Hej @Delta zrób to',
      caret: 11,
    });
  });

  it('adresatka z pierwszego @imienia', () => {
    const agents = [
      { id: 'alfa' as const, name: 'Alfa' },
      { id: 'delta' as const, name: 'Delta' },
    ];
    expect(addressedAgent('@Delta przenieś pliki', agents)).toBe('delta');
    expect(addressedAgent('bez adresu', agents)).toBeNull();
    expect(addressedAgent('@Zeta', agents)).toBeNull();
  });
});
