import { describe, expect, it } from 'vitest';
import { interpolate, pickPlural, pluralCategory, translate } from '../core';
import { formatBytes, formatDuration, formatMoney, formatRelative } from '../format';

const files = { one: '{n} plik', few: '{n} pliki', many: '{n} plików', other: '{n} pliku' };

describe('liczby mnogie PL przez Intl.PluralRules', () => {
  it('1 plik, 2 pliki, 5 plików, 22 pliki, 25 plików, 1,5 pliku', () => {
    const t = (n: number) => translate({ files }, {}, 'pl', 'files', { n });
    expect(t(1)).toBe('1 plik');
    expect(t(2)).toBe('2 pliki');
    expect(t(4)).toBe('4 pliki');
    expect(t(5)).toBe('5 plików');
    expect(t(12)).toBe('12 plików');
    expect(t(22)).toBe('22 pliki');
    expect(t(25)).toBe('25 plików');
    expect(t(0)).toBe('0 plików');
    expect(t(1.5)).toBe('1,5 pliku');
    expect(t(1000)).toBe('1000 plików');
  });

  it('kategorie EN: one / other', () => {
    expect(pluralCategory('en', 1)).toBe('one');
    expect(pluralCategory('en', 2)).toBe('other');
    expect(pickPlural({ one: 'file', other: 'files' }, 'en', 5)).toBe('files');
  });

  it('interpolacja i zapasowy słownik', () => {
    expect(interpolate('{name} pracuje', 'pl', { name: 'Delta' })).toBe('Delta pracuje');
    expect(interpolate('bez {x}', 'pl')).toBe('bez {x}');
    expect(translate({}, { a: 'z PL' }, 'en', 'a')).toBe('z PL');
    expect(translate({}, {}, 'pl', 'brak.klucza')).toBe('brak.klucza');
  });
});

describe('formaty pl-PL', () => {
  it('waluta, rozmiary, czasy', () => {
    expect(formatMoney('pl', { minor: 214, currency: 'PLN' }).replace(/\s/g, ' ')).toBe('2,14 zł');
    expect(formatMoney('en', { minor: 214, currency: 'PLN' })).toContain('2.14');
    expect(formatBytes('pl', 48_213).replace(/\s/g, ' ')).toBe('47 KB');
    expect(formatBytes('pl', 1_904).replace(/\s/g, ' ')).toBe('1,9 KB');
    expect(formatDuration('pl', 820)).toBe('820 ms');
    expect(formatDuration('pl', 2100)).toBe('2,1 s');
    expect(formatDuration('pl', 65_000)).toBe('1:05');
  });

  it('czas względny', () => {
    const now = Date.UTC(2026, 8, 30, 12);
    expect(formatRelative('pl', new Date(now - 2 * 60_000).toISOString(), now)).toBe('2 min temu');
    expect(formatRelative('pl', new Date(now - 10_000).toISOString(), now)).toBe('teraz');
  });
});
