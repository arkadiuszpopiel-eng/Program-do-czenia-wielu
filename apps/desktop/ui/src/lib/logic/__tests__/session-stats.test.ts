import { describe, expect, it } from 'vitest';
import { turn } from '../../api/fake/fixtures';
import { countWords, sessionStats, statsMarkdown } from '../session-stats';

const usage = (model: string, minor: number, latency: number) => ({
  input_tokens: 100,
  output_tokens: 40,
  cost: { minor, currency: 'PLN' as const },
  latency_ms: latency,
  provider: 'anthropic',
  model,
});

describe('session-stats', () => {
  it('słowa: polskie znaki, liczby, wyrazy z łącznikiem; pusty tekst', () => {
    expect(countWords('Zażółć gęślą jaźń — 2 razy, biało-czerwona!')).toBe(6);
    expect(countWords('   ')).toBe(0);
  });

  it('liczy wiadomości, słowa, tokeny, koszt, modele, narzędzia i inne gałęzie', () => {
    const path = [
      turn('u1', 's', null, 'user', 'Zaplanuj mi tydzień pracy', 0),
      turn('a1', 's', 'u1', 'alfa', 'Oto plan na pięć dni', 1000, {
        usage: usage('sonnet', 12, 800),
        tools: [{ id: 't', name: 'kalendarz', status: 'done' } as never],
      }),
      turn('u2', 's', 'a1', 'user', 'Dzięki', 2000),
      turn('a2', 's', 'u2', 'alfa', 'Proszę bardzo', 3000, {
        usage: usage('haiku', 3, 400),
      }),
    ];
    const s = sessionStats(path, 6);
    expect(s).toMatchObject({
      userMessages: 2,
      agentMessages: 2,
      userWords: 5,
      agentWords: 7,
      readingMinutes: 1,
      inputTokens: 200,
      outputTokens: 80,
      cost: { minor: 15, currency: 'PLN' },
      tools: 1,
      averageLatencyMs: 600,
      otherBranches: 2,
    });
    expect(s.models).toEqual([
      { name: 'anthropic · haiku', replies: 1 },
      { name: 'anthropic · sonnet', replies: 1 },
    ]);
    expect(s.first).toBe(path[0]?.created_at);
    expect(s.last).toBe(path[3]?.created_at);
  });

  it('pusta rozmowa: zera i brak kosztu', () => {
    const s = sessionStats([], 0);
    expect(s.readingMinutes).toBe(0);
    expect(s.cost).toBeNull();
    expect(s.averageLatencyMs).toBeNull();
    expect(s.first).toBeNull();
  });

  it('Markdown do schowka', () => {
    expect(statsMarkdown('Statystyki', [['Wiadomości', '4']])).toBe(
      '## Statystyki\n\n- **Wiadomości:** 4',
    );
  });
});
