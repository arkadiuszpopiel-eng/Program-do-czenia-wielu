import { describe, expect, it } from 'vitest';
import { findSentence, wordsOf } from '../sentence-match';

const slice = (text: string, s: ReturnType<typeof findSentence>) =>
  s ? text.slice(s.start, s.end) : null;

describe('findSentence (czytane zdanie, plan 43)', () => {
  it('znajduje zdanie mimo znaczników i wielkości liter', () => {
    const text = 'Plan na jutro\nAla ma kota.\nKot ma Alę! Koniec.';
    const span = findSentence(text, 'ala ma kota.');
    expect(slice(text, span)).toBe('Ala ma kota');
    expect(slice(text, findSentence(text, 'Kot ma Alę!'))).toBe('Kot ma Alę');
  });

  it('powtórzone zdanie: szuka od końca poprzedniego', () => {
    const text = 'Tak. Nie wiem. Tak.';
    const first = findSentence(text, 'Tak.');
    expect(first?.start).toBe(0);
    const second = findSentence(text, 'Nie wiem.', first?.nextWord ?? 0);
    const third = findSentence(text, 'Tak.', second?.nextWord ?? 0);
    expect(third?.start).toBe(text.lastIndexOf('Tak'));
  });

  it('opis kodu z rdzenia nie psuje dopasowania (słowa spoza ekranu pomijane)', () => {
    const text = 'Uruchom polecenie\nnpm run check\nw katalogu interfejsu.';
    const span = findSentence(text, 'Uruchom polecenie (kod na ekranie) w katalogu interfejsu.');
    expect(span?.start).toBe(0);
    expect(slice(text, span)?.endsWith('interfejsu')).toBe(true);
  });

  it('za słabe dopasowanie albo pusty tekst — brak wyróżnienia', () => {
    expect(findSentence('Zupełnie inny tekst na ekranie.', 'Ala ma kota i psa.')).toBeNull();
    expect(findSentence('', 'Ala.')).toBeNull();
    expect(findSentence('Ala.', '…')).toBeNull();
  });

  it('słowa: litery i cyfry Unicode, małe litery', () => {
    expect(wordsOf('Żółw, 3 KOTY!').map((w) => w.word)).toEqual(['żółw', '3', 'koty']);
  });
});
