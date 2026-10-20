import { describe, expect, it } from 'vitest';
import type { Turn, TurnStatus } from '../../api/types';
import type { RoutingModel } from '../../api/types-hub';
import { turn } from '../../api/fake/fixtures';
import {
  arenaBest,
  arenaMetrics,
  arenaStep,
  arenaTarget,
  defaultPair,
  modelLabel,
  type ArenaSlot,
} from '../arena';
import { buildTree, projectPath } from '../turn-tree';

const T0 = Date.UTC(2026, 9, 20, 10);

const model = (candidate: string, local = false): RoutingModel => ({
  candidate,
  provider_name: local ? 'Model lokalny' : 'Anthropic',
  model: candidate.slice(candidate.indexOf(':') + 1),
  local,
  rung: 'standard',
  automatic: 'standard',
});

const usage = (latency: number, cost: number, name = 'claude-opus-5-5') => ({
  input_tokens: 1200,
  output_tokens: 80,
  cost: { minor: cost, currency: 'PLN' as const },
  latency_ms: latency,
  provider: 'Anthropic',
  model: name,
});

describe('Arena — cel porównania', () => {
  const tree = buildTree([
    turn('u1', 's', null, 'user', 'Pytanie', T0),
    turn('a1', 's', 'u1', 'alfa', 'Odpowiedź', T0 + 1000),
    turn('c1', 's', 'a1', 'alfa', '…dalej', T0 + 2000),
  ]);
  const path = projectPath(tree);

  it('bez wskazania — ostatnia wiadomość użytkownika ścieżki', () => {
    expect(arenaTarget(path, tree.turns, null)?.id).toBe('u1');
    expect(arenaTarget([], {}, null)).toBeUndefined();
  });

  it('wiadomość użytkownika albo odpowiedź na nią (→ rodzic)', () => {
    expect(arenaTarget(path, tree.turns, 'u1')?.id).toBe('u1');
    expect(arenaTarget(path, tree.turns, 'a1')?.id).toBe('u1');
    // Kontynuacja (rodzic to odpowiedź agentki) — nie ma czego porównać.
    expect(arenaTarget(path, tree.turns, 'c1')).toBeUndefined();
    expect(arenaTarget(path, tree.turns, 'brak')).toBeUndefined();
  });
});

describe('Arena — para modeli', () => {
  it('lokalny i z chmury, gdy są oba', () => {
    const list = [model('anthropic:a'), model('openai:b'), model('local:q', true)];
    expect(defaultPair(list)).toEqual(['local:q', 'anthropic:a']);
  });

  it('inaczej dwa pierwsze; za mało modeli → null', () => {
    expect(defaultPair([model('anthropic:a'), model('openai:b')])).toEqual([
      'anthropic:a',
      'openai:b',
    ]);
    expect(defaultPair([model('anthropic:a')])).toBeNull();
    expect(modelLabel(model('anthropic:a'))).toBe('Anthropic · a');
  });
});

describe('Arena — kolejność odpowiedzi', () => {
  const slots = (...s: Partial<ArenaSlot>[]): ArenaSlot[] =>
    s.map((x, i) => ({ model: `p:m${i}`, turnId: null, error: null, ...x }));
  const statuses =
    (map: Record<string, TurnStatus>) =>
    (id: string): TurnStatus | undefined =>
      map[id];

  it('pierwsza startuje, druga czeka na koniec pierwszej', () => {
    expect(arenaStep(slots({}, {}), statuses({}), false)).toEqual({ kind: 'start', index: 0 });
    const running = slots({ turnId: 't1' }, {});
    expect(arenaStep(running, statuses({}), false)).toEqual({ kind: 'wait' });
    expect(arenaStep(running, statuses({ t1: 'streaming' }), false)).toEqual({ kind: 'wait' });
    expect(arenaStep(running, statuses({ t1: 'complete' }), false)).toEqual({
      kind: 'start',
      index: 1,
    });
  });

  it('błąd startu i błąd tury nie blokują drugiej odpowiedzi', () => {
    expect(arenaStep(slots({ error: 'Broker' }, {}), statuses({}), false)).toEqual({
      kind: 'start',
      index: 1,
    });
    expect(arenaStep(slots({ turnId: 't1' }, {}), statuses({ t1: 'error' }), false)).toEqual({
      kind: 'start',
      index: 1,
    });
  });

  it('koniec po obu; zatrzymanie nie startuje kolejnych', () => {
    const both = slots({ turnId: 't1' }, { turnId: 't2' });
    expect(arenaStep(both, statuses({ t1: 'complete', t2: 'cancelled' }), false)).toEqual({
      kind: 'done',
    });
    expect(arenaStep(slots({ turnId: 't1' }, {}), statuses({ t1: 'cancelled' }), true)).toEqual({
      kind: 'done',
    });
  });
});

describe('Arena — miary i porównanie', () => {
  const answer = (u: Turn['usage']): Turn => ({
    ...turn('a', 's', 'u', 'alfa', 'x', T0),
    usage: u,
  });

  it('miary z Usage; model zastępczy rozpoznany', () => {
    expect(arenaMetrics(answer(null), 'anthropic:claude-opus-5-5')).toBeNull();
    expect(arenaMetrics(undefined, 'anthropic:claude-opus-5-5')).toBeNull();
    const ok = arenaMetrics(answer(usage(900, 4)), 'anthropic:claude-opus-5-5');
    expect(ok).toMatchObject({ latencyMs: 900, outputTokens: 80, fallback: false });
    const other = arenaMetrics(answer(usage(900, 4, 'gpt-6-sol')), 'anthropic:claude-opus-5-5');
    expect(other?.fallback).toBe(true);
  });

  it('szybsza i tańsza; remis i brak danych → null', () => {
    const a = arenaMetrics(answer(usage(500, 9)), 'p:claude-opus-5-5');
    const b = arenaMetrics(answer(usage(800, 3)), 'p:claude-opus-5-5');
    expect(arenaBest([a, b])).toEqual({ faster: 0, cheaper: 1 });
    expect(arenaBest([a, a])).toEqual({ faster: null, cheaper: null });
    expect(arenaBest([a, null])).toEqual({ faster: null, cheaper: null });
  });

  it('odpowiedź zatrzymana (niedokończona) nie wygrywa porównania', () => {
    const a = arenaMetrics(answer(usage(900, 9)), 'p:claude-opus-5-5');
    const stopped: Turn = { ...answer(usage(200, 1)), status: 'cancelled' };
    const b = arenaMetrics(stopped, 'p:claude-opus-5-5');
    expect(a?.complete).toBe(true);
    expect(b).toMatchObject({ latencyMs: 200, complete: false });
    expect(arenaBest([a, b])).toEqual({ faster: null, cheaper: null });
  });
});
