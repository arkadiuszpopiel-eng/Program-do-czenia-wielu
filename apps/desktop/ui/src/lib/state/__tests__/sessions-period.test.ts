import { describe, expect, it } from 'vitest';
import { session } from '../../api/fake/fixtures';
import { inPeriod } from '../sessions.svelte';

describe('archiwum rozmów: okres', () => {
  const now = new Date(2026, 9, 15, 14, 0).getTime();
  const day = 86_400_000;
  const list = [
    session('a', 'Dziś rano', new Date(2026, 9, 15, 8, 0).getTime()),
    session('b', 'Wczoraj', now - day),
    session('c', 'Dwa tygodnie temu', now - 14 * day),
    session('d', 'Dwa miesiące temu', now - 60 * day),
  ];
  const ids = (p: Parameters<typeof inPeriod>[1]) => inPeriod(list, p, now).map((s) => s.id);

  it('filtruje po ostatniej aktywności', () => {
    expect(ids('all')).toEqual(['a', 'b', 'c', 'd']);
    expect(ids('today')).toEqual(['a']);
    expect(ids('week')).toEqual(['a', 'b']);
    expect(ids('month')).toEqual(['a', 'b', 'c']);
  });
});
