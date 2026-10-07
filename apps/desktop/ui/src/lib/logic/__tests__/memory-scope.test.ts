import { describe, expect, it } from 'vitest';
import { scopeTitle } from '../memory-scope';

describe('nazwa zakresu pamięci', () => {
  it('rodzaj i nazwa, gdy się różnią', () => {
    expect(scopeTitle('Sesja', 'Raport Q3')).toBe('Sesja: Raport Q3');
    expect(scopeTitle('Projekt', 'Projekt X')).toBe('Projekt X');
  });

  it('bez powtórzeń, gdy nazwa zaczyna się od rodzaju', () => {
    expect(scopeTitle('Globalna', 'Globalna')).toBe('Globalna');
    expect(scopeTitle('Agentka', 'Agentka beta')).toBe('Agentka beta');
    expect(scopeTitle('Global', 'global')).toBe('global');
  });

  it('pusta nazwa → sam rodzaj', () => {
    expect(scopeTitle('Sesja', '  ')).toBe('Sesja');
  });
});
