import { describe, expect, it } from 'vitest';
import { spokenText, sentencesOf } from '../../api/fake/read-aloud';
import { reading, voiceOf } from '../reading.svelte';
import type { Turn } from '../../api/types';

describe('reading (czytane zdanie, plan 43)', () => {
  it('zdanie po zdaniu; koniec tej tury czyści, koniec innej nie', () => {
    reading.apply({ turn_id: 's/1', index: 0, of: 2, text: 'Ala ma kota.' });
    expect([reading.turnId, reading.text, reading.index]).toEqual(['s/1', 'Ala ma kota.', 0]);
    reading.apply({ turn_id: 's/1', index: 1, of: 2, text: 'Kot ma Alę.' });
    expect(reading.text).toBe('Kot ma Alę.');
    reading.apply({ turn_id: 's/9', index: 3, of: 3, text: '' });
    expect(reading.turnId).toBe('s/1');
    reading.apply({ turn_id: 's/1', index: 2, of: 2, text: '' });
    expect(reading.turnId).toBeNull();
  });

  it('awatar mówi przy pisaniu i przy czytaniu; litery z czytanego zdania', () => {
    const turn = { id: 's/2', text: 'Cała odpowiedź.' } as Turn;
    expect(voiceOf(turn, false)).toEqual({ speaking: false, words: undefined });
    expect(voiceOf(turn, true)).toEqual({ speaking: true, words: 'Cała odpowiedź.' });
    reading.apply({ turn_id: 's/2', index: 0, of: 1, text: 'Czytane zdanie.' });
    expect(voiceOf(turn, false)).toEqual({ speaking: true, words: 'Czytane zdanie.' });
    reading.clear();
  });

  it('atrapa: tekst mówiony bez znaczników, kod zastąpiony opisem; podział na zdania', () => {
    const md = '## Plan\n**Ala** ma `kota`.\n```sh\nnpm i\n```\nKoniec!';
    expect(spokenText(md)).toBe('Plan Ala ma kota. (kod na ekranie) Koniec!');
    expect(sentencesOf('Raz. Dwa! Trzy')).toEqual(['Raz.', 'Dwa!', 'Trzy']);
    // Kropka w nazwie pliku i liczba porządkowa nie kończą zdania (jak `split_sentences`).
    expect(sentencesOf('Plik zarząd.dotx, gotowy. Od 3. maja tak.')).toEqual([
      'Plik zarząd.dotx, gotowy.',
      'Od 3. maja tak.',
    ]);
  });
});
