// Ostatni wynik wyzwalacza (plan P14, widok obowiązków): ostatnie uruchomienie z dziennika
// połączone z zadaniem, które z niego powstało — stan, wynik i raport agentki.
import type { TaskInfo, TriggerRunInfo } from '../api/types-tasks';

/** Jak skończyło się ostatnie uruchomienie — do etykiety i koloru. */
export type LastRunTone = 'ok' | 'running' | 'bad' | 'idle';

export interface LastRun {
  readonly at: string;
  /** `submitted`, `suppressed`, `deferred`, `failed` (z dziennika wyzwalacza). */
  readonly outcome: string;
  readonly detail: string | null;
  /** Zadanie z tego uruchomienia (`null` — nie powstało albo zniknęło z listy). */
  readonly task: TaskInfo | null;
  readonly tone: LastRunTone;
}

function toneOf(outcome: string, task: TaskInfo | null): LastRunTone {
  if (outcome === 'failed') return 'bad';
  if (outcome !== 'submitted' || !task) return 'idle';
  if (task.state !== 'done') return 'running';
  return task.result === 'succeeded' ? 'ok' : 'bad';
}

/** Ostatnie uruchomienie każdego wyzwalacza (klucz: identyfikator wyzwalacza). */
export function lastRuns(
  log: readonly TriggerRunInfo[],
  tasks: readonly TaskInfo[],
): ReadonlyMap<string, LastRun> {
  const byTask = new Map(tasks.map((task) => [task.id, task]));
  const result = new Map<string, LastRun>();
  for (const run of log) {
    const previous = result.get(run.trigger_id);
    if (previous && previous.at > run.at) continue;
    const task = run.task_id ? (byTask.get(run.task_id) ?? null) : null;
    result.set(run.trigger_id, {
      at: run.at,
      outcome: run.outcome,
      detail: run.detail,
      task,
      tone: toneOf(run.outcome, task),
    });
  }
  return result;
}
