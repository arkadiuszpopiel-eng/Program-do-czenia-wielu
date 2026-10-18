import { describe, expect, it } from 'vitest';
import { SEEDS, seedItem } from '../fake/engine-seeds';
// Dane produkcyjne kart — atrapa nie może się od nich odkleić.
import cards from '../../../../../../../crates/app-models/data/license-cards.json';

describe('atrapa: karty licencji (C5.7)', () => {
  it('każda pozycja atrapy ma kartę licencji z adresem i uwagami', () => {
    for (const seed of SEEDS) {
      const card = seedItem(seed).license_card;
      expect(card, seed[0]).not.toBeNull();
      expect(card?.url, seed[0]).toMatch(/^https:\/\//);
      expect(card?.notes.pl.length ?? 0, seed[0]).toBeGreaterThan(20);
      if (card && !card.confirmed)
        expect(card.notes.pl).toContain('Do potwierdzenia przez człowieka');
    }
  });

  it('identyfikator licencji, użycie komercyjne i potwierdzenie jak w danych produkcyjnych', () => {
    const prod: Record<string, { id: string; commercial: string; confirmed: boolean }> = cards;
    for (const seed of SEEDS) {
      const card = seedItem(seed).license_card;
      const real = prod[seed[0]];
      expect(real, seed[0]).toBeDefined();
      expect(card?.id, seed[0]).toBe(real?.id);
      expect(card?.commercial, seed[0]).toBe(real?.commercial);
      expect(card?.confirmed, seed[0]).toBe(real?.confirmed);
      // Jak w Rust: karta potwierdzona tylko przy potwierdzonej pozycji.
      if (card?.confirmed) expect(seed[7], seed[0]).toBe(true);
    }
  });

  it('głosy mają osobne wpisy, a mowa — sprawę znaku wodnego', () => {
    const card = (id: string) => {
      const seed = SEEDS.find((s) => s[0] === id);
      if (!seed) throw new Error(`brak pozycji ${id}`);
      return seedItem(seed).license_card;
    };
    expect(card('supertonic-3')?.voices.map((v) => v.id)).toEqual([
      'F1',
      'F2',
      'F3',
      'F4',
      'F5',
      'M1',
      'M2',
      'M3',
      'M4',
      'M5',
    ]);
    expect(card('supertonic-3')?.commercial).toBe('conditional');
    expect(card('piper-pl_PL-gosia-medium')?.voices).toHaveLength(1);
    expect(card('sidecar-pocket-tts')?.voices).toHaveLength(2);
    for (const id of ['supertonic-3', 'piper-pl_PL-gosia-medium', 'sidecar-pocket-tts']) {
      expect(card(id)?.notes.pl, id).toContain('Znak wodny');
      expect(card(id)?.notes.en, id).toContain('watermark');
    }
    expect(card('silero-vad')?.notes.pl).not.toContain('Znak wodny');
  });
});
