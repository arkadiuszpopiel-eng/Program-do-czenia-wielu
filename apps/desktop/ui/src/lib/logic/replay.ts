// Replay krok po kroku: scalanie przebiegów i kroków z komendy `agents_runs` i zdarzeń na żywo
// (`AgentRunUpdated`, `AgentStep`), kursor odtwarzania. Czyste funkcje (testy w __tests__).
import type { AgentRun, AgentRunDetail, ReplayStatus, ReplayStep } from '../api/types';

/** Kroki w kolejności (numer, potem czas — wiadomości w trakcie mają numer bieżącego kroku). */
export function sortSteps(steps: readonly ReplayStep[]): ReplayStep[] {
  return [...steps].sort((a, b) => a.n - b.n || a.at_ms - b.at_ms);
}

/** Wstawia albo zastępuje krok (po `id`). */
export function upsertStep(steps: readonly ReplayStep[], step: ReplayStep): ReplayStep[] {
  return sortSteps([...steps.filter((s) => s.id !== step.id), step]);
}

/** Nagłówek przebiegu z UI — nowy przebieg trafia na koniec listy (kroki puste). */
export function applyRun(runs: readonly AgentRunDetail[], run: AgentRun): AgentRunDetail[] {
  const index = runs.findIndex((r) => r.run.id === run.id);
  if (index < 0) return [...runs, { run, steps: [] }];
  return runs.map((r, i) => (i === index ? { run, steps: r.steps } : r));
}

/** Krok przebiegu z UI (krok przebiegu nieznanego jeszcze w liście jest pomijany). */
export function applyStep(
  runs: readonly AgentRunDetail[],
  runId: string,
  step: ReplayStep,
): AgentRunDetail[] {
  return runs.map((r) =>
    r.run.id === runId ? { run: r.run, steps: upsertStep(r.steps, step) } : r,
  );
}

/** Oznacza krok jako cofnięty (po tokenie). */
export function markUndone(runs: readonly AgentRunDetail[], token: string): AgentRunDetail[] {
  return runs.map((r) => ({
    run: r.run,
    steps: r.steps.map((s) => (s.undo_token === token ? { ...s, undone: true } : s)),
  }));
}

/** Kroki widoczne przy kursorze odtwarzania (`null` = wszystkie). */
export function visibleSteps(steps: readonly ReplayStep[], cursor: number | null): ReplayStep[] {
  return cursor === null ? [...steps] : steps.slice(0, Math.max(0, cursor) + 1);
}

/** Następna pozycja kursora (`null` = koniec odtwarzania, pokaż wszystko). */
export function stepCursor(cursor: number | null, delta: number, total: number): number | null {
  if (total === 0) return null;
  const from = cursor ?? (delta > 0 ? -1 : total);
  const next = from + delta;
  if (next < 0) return 0;
  if (next >= total) return null;
  return next;
}

/** Stan kroku → wariant wizualny (kolor + ikona + tekst — nigdy sam kolor). */
export function statusTone(status: ReplayStatus): 'ok' | 'warn' | 'error' | 'busy' | 'muted' {
  switch (status) {
    case 'ok':
      return 'ok';
    case 'running':
    case 'waiting_approval':
      return 'busy';
    case 'needs_confirmation':
      return 'warn';
    case 'denied':
    case 'failed':
      return 'error';
    case 'cancelled':
      return 'muted';
  }
}

/** Czy przebieg trwa. */
export function isActive(run: AgentRun): boolean {
  return run.state === 'running' || run.state === 'waiting_approval' || run.state === 'paused';
}
