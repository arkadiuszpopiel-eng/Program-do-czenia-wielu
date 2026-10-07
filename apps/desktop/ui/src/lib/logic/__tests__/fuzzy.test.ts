import { describe, expect, it } from 'vitest';
import { fuzzyRank, fuzzyScore, normalize } from '../fuzzy';

describe('wyszukiwanie rozmyte', () => {
  it('ignoruje wielkość liter i polskie znaki', () => {
    expect(normalize('Pamięć ŁÓDŹ')).toBe('pamiec lodz');
    expect(fuzzyScore('pamiec', 'Pamięć')).toBe(1);
    expect(fuzzyScore('lodz', 'Łódź')).toBe(1);
  });

  it('kolejność: prefiks > początek słowa > środek > podciąg', () => {
    const prefix = fuzzyScore('ust', 'Ustawienia');
    const word = fuzzyScore('sesj', 'Nowa sesja');
    const middle = fuzzyScore('awi', 'Ustawienia');
    const subseq = fuzzyScore('nwrz', 'Nowa rozmowa');
    expect(prefix).toBeGreaterThan(word);
    expect(word).toBeGreaterThan(middle);
    expect(middle).toBeGreaterThan(subseq);
    expect(subseq).toBeGreaterThan(0);
  });

  it('brak dopasowania = 0; puste zapytanie = 1', () => {
    expect(fuzzyScore('xyz', 'Ustawienia')).toBe(0);
    expect(fuzzyScore('  ', 'cokolwiek')).toBe(1);
  });

  it('wiele słów — każde musi pasować', () => {
    expect(fuzzyScore('panel sesje', 'Panel Sesje')).toBeGreaterThan(0.9);
    expect(fuzzyScore('panel xyz', 'Panel Sesje')).toBe(0);
  });

  it('ranking z uwzględnieniem słów kluczowych i limitu', () => {
    const items = [
      { label: 'Przełącz motyw', keywords: ['dark', 'ciemny'] },
      { label: 'Ustawienia', keywords: [] },
      { label: 'Tryb skupienia', keywords: ['focus'] },
    ];
    const ranked = fuzzyRank(items, 'ciemny', (i) => i);
    expect(ranked.map((r) => r.item.label)).toEqual(['Przełącz motyw']);
    expect(fuzzyRank(items, '', (i) => i, 2)).toHaveLength(2);
  });

  it('≤ 16 ms na znak dla 1000 pozycji', () => {
    const items = Array.from({ length: 1000 }, (_, i) => ({
      label: `Sesja numer ${i} — raport kwartalny`,
    }));
    const start = performance.now();
    for (const q of ['r', 'ra', 'rap', 'rapo']) fuzzyRank(items, q, (i) => i);
    expect((performance.now() - start) / 4).toBeLessThan(16);
  });
});
