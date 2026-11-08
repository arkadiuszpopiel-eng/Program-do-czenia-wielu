import { describe, expect, it } from 'vitest';
import { replayStep } from '../../api/fake/api-agents';
import type { AgentRun, AgentRunDetail } from '../../api/types';
import {
  applyRun,
  applyStep,
  isActive,
  markUndone,
  sortSteps,
  statusTone,
  stepCursor,
  upsertStep,
  visibleSteps,
} from '../replay';

const run = (id: string, state: AgentRun['state'] = 'running'): AgentRun => ({
  id,
  session_id: 's1',
  turn_id: 's1:t2',
  agent: 'delta',
  goal: 'Uporządkuj Pobrane',
  workdir: 'C:\\w',
  state,
  started_at: '2026-10-01T10:00:00Z',
  finished_at: null,
  summary: null,
  usage: {
    steps: 0,
    tool_calls: 0,
    input_tokens: 0,
    output_tokens: 0,
    cost: { minor: 0, currency: 'PLN' },
    elapsed_ms: 0,
  },
  budget: { max_steps: 40, max_minutes: 15, max_cost: null },
});

describe('Replay: scalanie kroków i odtwarzanie', () => {
  it('upsert kroku zastępuje po id i sortuje po numerze i czasie', () => {
    const a = replayStep('r', 2, { title: 'Zapis', at_ms: 20 });
    const b = replayStep('r', 1, { title: 'Plan', at_ms: 0 });
    let steps = upsertStep([a], b);
    expect(steps.map((s) => s.n)).toEqual([1, 2]);
    steps = upsertStep(steps, { ...a, status: 'ok', output: 'zapisano' });
    expect(steps).toHaveLength(2);
    expect(steps[1]?.status).toBe('ok');
    const steer = { ...replayStep('r', 1, { title: 'Wiadomość', at_ms: 5 }), id: 'r:m1' };
    expect(sortSteps([...steps, steer]).map((s) => s.id)).toEqual(['r:s1', 'r:m1', 'r:s2']);
  });

  it('nagłówki i kroki z UI: nowy przebieg na końcu, krok trafia do swojego przebiegu', () => {
    let runs: AgentRunDetail[] = [];
    runs = applyRun(runs, run('r1'));
    runs = applyRun(runs, run('r2'));
    runs = applyStep(runs, 'r2', replayStep('r2', 1, { title: 'Plan' }));
    runs = applyStep(runs, 'nieznany', replayStep('x', 1, { title: 'X' }));
    expect(runs.map((r) => [r.run.id, r.steps.length])).toEqual([
      ['r1', 0],
      ['r2', 1],
    ]);
    runs = applyRun(runs, run('r2', 'completed'));
    expect(runs[1]?.run.state).toBe('completed');
    expect(runs[1]?.steps).toHaveLength(1);
    expect(isActive(runs[0]!.run)).toBe(true);
    expect(isActive(runs[1]!.run)).toBe(false);
  });

  it('cofnięcie oznacza krok po tokenie', () => {
    const step = replayStep('r1', 3, { title: 'Zapis', undo_token: 's1:u7' });
    const runs = markUndone([{ run: run('r1'), steps: [step] }], 's1:u7');
    expect(runs[0]?.steps[0]?.undone).toBe(true);
  });

  it('kursor: od początku, krok po kroku, koniec = wszystkie', () => {
    const steps = [1, 2, 3].map((n) => replayStep('r', n, { title: `k${n}` }));
    expect(stepCursor(null, 1, 3)).toBe(0);
    expect(stepCursor(0, 1, 3)).toBe(1);
    expect(stepCursor(2, 1, 3)).toBeNull();
    expect(stepCursor(0, -1, 3)).toBe(0);
    expect(stepCursor(null, -1, 3)).toBe(2);
    expect(stepCursor(null, 1, 0)).toBeNull();
    expect(visibleSteps(steps, 1)).toHaveLength(2);
    expect(visibleSteps(steps, null)).toHaveLength(3);
  });

  it('stan kroku ma wariant (ikona + tekst, nie sam kolor)', () => {
    expect(statusTone('ok')).toBe('ok');
    expect(statusTone('denied')).toBe('error');
    expect(statusTone('waiting_approval')).toBe('busy');
    expect(statusTone('needs_confirmation')).toBe('warn');
    expect(statusTone('cancelled')).toBe('muted');
  });
});
