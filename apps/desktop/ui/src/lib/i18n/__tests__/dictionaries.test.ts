import { describe, expect, it } from 'vitest';
import type { Message } from '../core';
import { en } from '../en';
import { pl } from '../pl';

const placeholders = (m: Message): string[] => {
  const text = typeof m === 'string' ? m : Object.values(m).join(' ');
  return [...new Set(text.match(/\{\w+\}/g) ?? [])].sort();
};

describe('słowniki PL/EN', () => {
  it('te same klucze', () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(pl).sort());
  });

  it('te same zmienne {…} w PL i EN', () => {
    for (const key of Object.keys(pl) as (keyof typeof pl)[]) {
      expect(placeholders(en[key]), key).toEqual(placeholders(pl[key]));
    }
  });

  it('formy mnogie PL mają one/few/many', () => {
    for (const [key, value] of Object.entries(pl)) {
      if (typeof value === 'string') continue;
      expect(Object.keys(value).sort(), key).toEqual(['few', 'many', 'one', 'other']);
    }
  });

  it('agentki w rodzaju żeńskim (wybrane komunikaty)', () => {
    expect(pl['agents.hint']).toContain('do niej');
    expect(pl['ob.voice.desc']).toMatch(/Dobrałyśmy/);
  });
});
