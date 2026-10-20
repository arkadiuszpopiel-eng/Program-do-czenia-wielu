import { describe, expect, it } from 'vitest';
import type { Money, TurnUsage } from '../../api/types';
import { costText } from '../turn-cost';

const money = (m: Money) => `${(m.minor / 100).toFixed(2).replace('.', ',')} zł`;
const usage = (minor: number, micro: number | null): TurnUsage => ({
  input_tokens: 1744,
  output_tokens: 222,
  cost: { minor, currency: 'PLN' },
  cost_micro_pln: micro,
  latency_ms: 3600,
  provider: 'Anthropic',
  model: 'claude-haiku-5-5',
});

describe('koszt tury', () => {
  it('kwota, ułamek grosza i koszt nieznany', () => {
    expect(costText(usage(12, 120_000), money, '?')).toBe('0,12 zł');
    expect(costText(usage(0, 1_100), money, '?')).toBe('< 0,01 zł');
    expect(costText(usage(0, null), money, 'koszt nieznany')).toBe('koszt nieznany');
    expect(costText(usage(0, 0), money, '?')).toBe('0,00 zł');
  });
});
