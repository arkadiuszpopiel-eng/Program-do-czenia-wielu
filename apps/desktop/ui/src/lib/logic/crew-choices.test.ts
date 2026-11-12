import { describe, expect, it } from 'vitest';
import { crewChoices } from './crew-choices';
import { addressedAgent } from './composer';
import type { AgentState } from '../api/types';

const person = (id: string, role_ids: string[]): AgentState => ({
  id,
  role_ids,
  status: 'idle',
  activity: null,
});

describe('adresatki z rzeczywistej obsady rozmowy', () => {
  it('pozostawia cztery wbudowane wybory także przy pustej obsadzie', () => {
    expect(crewChoices([]).map((a) => a.id)).toEqual(['alfa', 'beta', 'gama', 'delta']);
  });
  it('dodaje własną personę tylko po przypisaniu roli i bez duplikatów', () => {
    const choices = crewChoices([
      person('luna', []),
      person('zofia-2', ['czytelniczka']),
      person('alfa', ['planistka']),
      person('zofia-2', ['czytelniczka']),
    ]);
    expect(choices.map((a) => a.id)).toEqual(['alfa', 'beta', 'gama', 'delta', 'zofia-2']);
    expect(addressedAgent('@luna zrób to', choices)).toBeNull();
    expect(addressedAgent('@zofia-2 sprawdź', choices)).toBe('zofia-2');
  });
  it('obsługuje cyfry, podkreślenia i polskie znaki bez łapania adresów poczty', () => {
    const choices = crewChoices([person('łucja_2', ['czytelniczka'])]);
    expect(addressedAgent('Hej @ŁUCJA_2, sprawdź', choices)).toBe('łucja_2');
    expect(addressedAgent('mail@łucja_2.example', choices)).toBeNull();
  });
});
