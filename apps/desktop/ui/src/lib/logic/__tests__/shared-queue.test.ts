import { describe, expect, it } from 'vitest';
import type { Account, ModelInfo } from '../../api/types-hub';
import { entryKey, entryModels, entryOf, fromAccountQueues } from '../shared-queue';

const model = (id: string, name: string): ModelInfo =>
  ({ id, name, kinds: ['chat'], purpose: 'chat' }) as unknown as ModelInfo;

const account = (id: string, label: string, models: ModelInfo[], chat: string[] = []) =>
  ({ id, label, models, queues: chat.length ? { chat } : {} }) as unknown as Account;

describe('wspólna kolejka modeli', () => {
  it('klucz pozycji zachowuje ukośniki w nazwie modelu', () => {
    const entry = { account_id: 'acc-or', model: 'openai/gpt-6-luna' };
    expect(entryKey(entry)).toBe('acc-or/openai/gpt-6-luna');
    expect(entryOf(entryKey(entry))).toEqual(entry);
    expect(entryOf('bez-konta')).toBeNull();
    expect(entryOf('/model')).toBeNull();
    expect(entryOf('acc-1/')).toBeNull();
  });

  it('opis kafelka to model konta; brakujący model i usunięte konto nie mają opisu', () => {
    const accounts = [account('acc-or', 'OpenRouter', [model('openai/gpt-6-luna', 'GPT-6 Luna')])];
    const map = entryModels(
      ['acc-or/openai/gpt-6-luna', 'acc-or/nie-ma', 'acc-usuniete/gpt'],
      accounts,
    );
    expect(map.get('acc-or/openai/gpt-6-luna')?.name).toBe('GPT-6 Luna');
    expect(map.has('acc-or/nie-ma')).toBe(false);
    expect(map.has('acc-usuniete/gpt')).toBe(false);
  });

  it('składa kolejkę z kolejek kont po kolei, bez powtórzeń i najwyżej do 10', () => {
    const accounts = [
      account('acc-a', 'A', [], ['m1', 'm2']),
      account('acc-b', 'B', [], ['m1']),
      account('acc-c', 'C', [], []),
    ];
    expect(fromAccountQueues(accounts, 'chat')).toEqual([
      { account_id: 'acc-a', model: 'm1' },
      { account_id: 'acc-a', model: 'm2' },
      { account_id: 'acc-b', model: 'm1' },
    ]);
    const many = [
      account(
        'acc-x',
        'X',
        [],
        Array.from({ length: 12 }, (_, i) => `m${i}`),
      ),
    ];
    expect(fromAccountQueues(many, 'chat')).toHaveLength(10);
    expect(fromAccountQueues(accounts, 'code')).toEqual([]);
  });
});
