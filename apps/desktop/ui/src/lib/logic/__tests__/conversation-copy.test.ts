import { describe, expect, it } from 'vitest';
import { turn } from '../../api/fake/fixtures';
import { conversationMarkdown } from '../conversation-copy';

const labels = {
  you: 'Ty',
  agent: (id: string) => (id === 'alfa' ? 'Alfa' : id),
  when: () => '16.10.2026 01:00',
  aiNote: 'Wiadomości oznaczone „AI” wygenerowała sztuczna inteligencja.',
};

describe('conversation-copy', () => {
  it('tytuł, nota AI, autorzy z oznaczeniem AI i modelem; puste tury pominięte', () => {
    const path = [
      turn('u1', 's', null, 'user', 'Ile to 2+2?', 0),
      turn('a1', 's', 'u1', 'alfa', '2 + 2 = **4**', 1000, {
        usage: {
          input_tokens: 1,
          output_tokens: 1,
          cost: { minor: 0, currency: 'PLN' },
          latency_ms: 10,
          provider: 'local',
          model: 'qwen3',
        },
      }),
      turn('a2', 's', 'a1', 'alfa', '   ', 2000),
    ];
    expect(conversationMarkdown(' Rachunki ', path, labels)).toBe(
      [
        '# Rachunki',
        '> Wiadomości oznaczone „AI” wygenerowała sztuczna inteligencja.',
        '---',
        '### Ty · 16.10.2026 01:00',
        'Ile to 2+2?',
        '---',
        '### Alfa · AI, qwen3 · 16.10.2026 01:00',
        '2 + 2 = **4**',
      ].join('\n\n') + '\n',
    );
  });

  it('bez odpowiedzi agentek — bez noty AI', () => {
    const md = conversationMarkdown('Szkic', [turn('u1', 's', null, 'user', 'Notatka', 0)], labels);
    expect(md).not.toContain('sztuczna inteligencja');
  });
});
