import { describe, expect, it } from 'vitest';
import type { TaskInfo, TriggerRunInfo } from '../../api/types-tasks';
import { lastRuns } from '../trigger-runs';

const run = (
  trigger: string,
  at: string,
  outcome: string,
  task: string | null,
): TriggerRunInfo => ({
  at,
  trigger_id: trigger,
  cause: 'ręcznie',
  outcome,
  task_id: task,
  detail: outcome === 'failed' ? 'brak modelu' : null,
});

const task = (id: string, state: TaskInfo['state'], result: TaskInfo['result']): TaskInfo =>
  ({ id, state, result, summary: result === 'succeeded' ? 'Raport.' : null }) as TaskInfo;

describe('ostatni wynik wyzwalacza', () => {
  it('bierze najnowsze uruchomienie i łączy je z zadaniem', () => {
    const runs = lastRuns(
      [
        run('a', '2026-10-12T06:00:00.000Z', 'submitted', 't1'),
        run('a', '2026-10-12T08:00:00.000Z', 'submitted', 't2'),
        run('b', '2026-10-12T07:00:00.000Z', 'submitted', 't3'),
      ],
      [task('t1', 'done', 'failed'), task('t2', 'done', 'succeeded'), task('t3', 'running', null)],
    );
    expect(runs.get('a')?.task?.id).toBe('t2');
    expect(runs.get('a')?.tone).toBe('ok');
    expect(runs.get('a')?.task?.summary).toBe('Raport.');
    expect(runs.get('b')?.tone).toBe('running');
    expect(runs.has('c')).toBe(false);
  });

  it('kolejność wpisów dziennika nie ma znaczenia', () => {
    const runs = lastRuns(
      [
        run('a', '2026-10-12T08:00:00.000Z', 'submitted', 't2'),
        run('a', '2026-10-12T06:00:00.000Z', 'submitted', 't1'),
      ],
      [task('t1', 'done', 'succeeded'), task('t2', 'done', 'cancelled')],
    );
    expect(runs.get('a')?.task?.id).toBe('t2');
    expect(runs.get('a')?.tone).toBe('bad');
  });

  it('rozróżnia błąd uruchomienia, wstrzymanie i brak zadania', () => {
    const runs = lastRuns(
      [
        run('f', '2026-10-12T08:00:00.000Z', 'failed', null),
        run('s', '2026-10-12T08:00:00.000Z', 'suppressed', null),
        run('g', '2026-10-12T08:00:00.000Z', 'submitted', 'zniknęło'),
      ],
      [],
    );
    expect(runs.get('f')?.tone).toBe('bad');
    expect(runs.get('f')?.detail).toBe('brak modelu');
    expect(runs.get('s')?.tone).toBe('idle');
    expect(runs.get('g')?.task).toBeNull();
    expect(runs.get('g')?.tone).toBe('idle');
  });
});
