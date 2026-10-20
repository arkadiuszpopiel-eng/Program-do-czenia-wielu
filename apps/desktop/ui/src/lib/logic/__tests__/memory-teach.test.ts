import { describe, expect, it, vi } from 'vitest';
import {
  TEACH_LIMITS,
  chunkText,
  isExternal,
  oneLine,
  paragraphs,
  scopeOfKey,
  teachScopes,
  undoTeach,
  capInput,
} from '../memory-teach';

const limits = (chunkChars: number, maxInputChars = 100_000, maxFacts = 1_000) => ({
  chunkChars,
  maxInputChars,
  maxFacts,
});

describe('Naucz Alfę — podział na fakty', () => {
  it('jedna linia bez znaków sterujących i niewidocznych', () => {
    expect(oneLine('A\n- (global) B\r\n\tC\u0000\u202Ed\u200B')).toBe('A - (global) B Cd');
  });

  it('akapity oddzielone pustą linią', () => {
    expect(paragraphs('Pierwszy\nciąg\n \nDrugi\n\n\n')).toEqual(['Pierwszy ciąg', 'Drugi']);
    expect(paragraphs(' \n\n\t')).toEqual([]);
  });

  it('krótkie akapity łączone w jeden fakt', () => {
    expect(chunkText('Ala ma kota.\n\nKot ma Alę.')).toEqual({
      pieces: ['Ala ma kota. Kot ma Alę.'],
      truncated: false,
      duplicates: 0,
    });
  });

  it('limit znaków fragmentu: zdania, słowa, znaki — bez gubienia treści', () => {
    const text = Array.from({ length: 40 }, (_, i) => `Zdanie ${i} o pamięci.`).join(' ');
    const c = chunkText(text, limits(60));
    expect(c.pieces.length).toBeGreaterThan(1);
    expect(c.pieces.every((p) => Array.from(p).length <= 60)).toBe(true);
    expect(c.pieces.join(' ')).toBe(text);
    const word = 'ąbcdęfghijklmnoprstuwyzżź';
    const d = chunkText(`Krótkie. ${word} koniec`, limits(10));
    expect(d.pieces.every((p) => Array.from(p).length <= 10)).toBe(true);
    expect(d.pieces.join('').replaceAll(' ', '')).toBe(`Krótkie.${word}koniec`);
  });

  it('limity wejścia i liczby faktów ucinają treść', () => {
    expect(chunkText('ąę '.repeat(100), limits(1_000, 50)).truncated).toBe(true);
    const many = Array.from({ length: 20 }, (_, i) => `Akapit ${i} unikalny.`).join('\n\n');
    const c = chunkText(many, limits(25, 100_000, 5));
    expect(c.pieces).toHaveLength(5);
    expect(c.truncated).toBe(true);
  });

  it('powtórzenia pomijane bez względu na wielkość liter', () => {
    const c = chunkText('Ten sam fakt.\n\nTEN SAM FAKT.\n\nInny fakt.', limits(14));
    expect(c.pieces).toEqual(['Ten sam fakt.', 'Inny fakt.']);
    expect(c.duplicates).toBe(1);
  });

  it('limity jak w rdzeniu', () => {
    expect(TEACH_LIMITS).toEqual({ chunkChars: 800, maxInputChars: 100_000, maxFacts: 128 });
    expect(chunkText('').pieces).toEqual([]);
  });
});

describe('Naucz Alfę — zakresy i cofnięcie', () => {
  it('treść z zewnątrz tylko w sesji, rozmowa także szerzej', () => {
    expect(isExternal('text')).toBe(true);
    expect(isExternal('file')).toBe(true);
    expect(isExternal('conversation')).toBe(false);
    expect(teachScopes('file')).toEqual(['session']);
    expect(teachScopes('conversation')).toEqual(['session', 'project', 'global']);
    expect(scopeOfKey('session:s-1')).toBe('session');
    expect(scopeOfKey('project:p-x')).toBe('project');
    expect(scopeOfKey('global')).toBe('global');
  });

  it('cofnięcie zapomina każdy fakt i liczy błędy', async () => {
    const forget = vi.fn((id: string) =>
      id === 'b' ? Promise.reject(new Error('nie ma')) : Promise.resolve(),
    );
    expect(await undoTeach(['a', 'b', 'c'], forget)).toEqual({
      removed: 2,
      failed: 1,
      left: ['b'],
    });
    expect(forget).toHaveBeenCalledTimes(3);
  });

  it('bardzo długi tekst ucinany przed podziałem', () => {
    const lim = limits(50, 100, 10);
    const raw = `${'a '.repeat(30)}${'b'.repeat(1_000_000)}`;
    expect(capInput(raw, lim).length).toBe(2 * (100 + 20_000));
    const out = chunkText(raw, lim);
    expect(out.truncated).toBe(true);
    expect(out.pieces.every((p) => Array.from(p).length <= 50)).toBe(true);
    expect(capInput('krótki', lim)).toBe('krótki');
  });
});
