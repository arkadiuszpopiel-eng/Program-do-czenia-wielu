import { describe, expect, it } from 'vitest';
import { estimate, levelOf, orderByRung, paramsBillions, rungByName, rungFor } from '../routing';

describe('dobór modelu według trudności (interfejs)', () => {
  it('szczebel z nazwy jak w rdzeniu', () => {
    expect(rungByName('bielik-4.5b-v3.0-instruct-q8_0')).toBe('light');
    expect(rungByName('qwen3-27b-instruct')).toBe('standard');
    expect(rungByName('llama-3.1-405b')).toBe('strong');
    expect(rungByName('gpt-4o-mini')).toBe('light');
    expect(rungByName('gpt-6-luna')).toBe('standard');
    expect(rungByName('gpt-6-sol')).toBe('strong');
    expect(rungByName('claude-opus-5-5')).toBe('strong');
    expect(paramsBillions('gpt-6-sol')).toBeNull();
  });

  it('ocena: krótkie łatwe, wyjaśnienie średnie, projekt z krokami trudny', () => {
    expect(levelOf(estimate('Która jest godzina?').score, 'balanced')).toBe('easy');
    expect(estimate('Która jest godzina?').reasons).toBe('krótka, prosta prośba');
    expect(levelOf(estimate('Wyjaśnij, jak działa fotosynteza.').score, 'balanced')).toBe('medium');
    expect(levelOf(estimate('Wyjaśnij, jak działa fotosynteza.').score, 'economy')).toBe('easy');
    const hard = estimate(
      'Zaprojektuj architekturę usługi: najpierw wymagania, następnie model danych, a potem wdrożenie.',
    );
    expect(levelOf(hard.score, 'balanced')).toBe('hard');
    expect(hard.reasons).toContain('rozumowanie');
    expect(estimate('To naprawdę dobre').reasons).toBe('krótka, prosta prośba');
  });

  it('kolejność od szczebla startowego, lokalne pierwsze przy remisie', () => {
    const list = [
      { id: 'opus', rung: 'strong' as const, local: false },
      { id: 'luna', rung: 'standard' as const, local: false },
      { id: 'qwen', rung: 'standard' as const, local: true },
      { id: 'bielik', rung: 'light' as const, local: true },
    ];
    expect(orderByRung(list, rungFor('easy'), true).map((x) => x.id)).toEqual([
      'bielik',
      'qwen',
      'luna',
      'opus',
    ]);
    expect(orderByRung(list, 'standard', false).map((x) => x.id)).toEqual([
      'luna',
      'qwen',
      'opus',
      'bielik',
    ]);
    expect(orderByRung(list, 'strong', true).map((x) => x.id)).toEqual([
      'opus',
      'qwen',
      'luna',
      'bielik',
    ]);
  });
});
