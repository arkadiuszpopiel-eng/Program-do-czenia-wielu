// Tryb „Plan” (plan 50 §4.1, Z.5): rozmowa na L0 — agentka tylko czyta i przedstawia plan. Pod
// planem trzy odpowiedzi jak w Claude Code: „Tak, i działaj automatycznie” (rozmowa na L3),
// „Tak, zatwierdzam ręcznie” (L1), „Nie, planuj dalej”. Podniesienie poziomu rozmowy potwierdza
// się w oknie Sędziego Cieni; dopiero gdy Sędzia je przyjmie, agentka dostaje polecenie wykonania.
import type { AutonomyLevel, Turn } from '../api/types';
import type { BrokerIntentResult } from '../api/types-hub';
import { watchRaise } from './raise-watch';

/** Poziom rozmowy po akceptacji planu. */
export const PLAN_LEVELS = { auto: 'L3', manual: 'L1' } as const satisfies Record<
  string,
  AutonomyLevel
>;
export type PlanAccept = keyof typeof PLAN_LEVELS;

/** Trzy odpowiedzi pod ostatnią, zakończoną odpowiedzią agentki w rozmowie w trybie Plan. */
export function showsPlanActions(
  turn: Turn,
  last: Turn | undefined,
  autonomy: AutonomyLevel | undefined,
): boolean {
  return (
    autonomy === 'L0' &&
    turn.id === last?.id &&
    turn.author !== 'user' &&
    turn.status === 'complete' &&
    turn.approval === null
  );
}

export interface PlanIo {
  readonly request: (level: AutonomyLevel) => Promise<BrokerIntentResult>;
  /** Poziom obowiązujący rozmowę (`null` — nieznany). */
  readonly readLevel: () => Promise<AutonomyLevel | null>;
  /** Polecenie wykonania planu (wysyłane, gdy poziom już obowiązuje). */
  readonly execute: () => Promise<void>;
  readonly notify: (event: 'requested' | 'started', level: AutonomyLevel) => void;
}

let stopWatch: (() => void) | null = null;

/** Akceptacja planu: poziom rozmowy, a po jego przyjęciu przez Sędziego — wykonanie. */
export async function acceptPlan(choice: PlanAccept, io: PlanIo): Promise<void> {
  const level = PLAN_LEVELS[choice];
  const result = await io.request(level);
  stopWatch?.();
  stopWatch = null;
  const start = async () => {
    io.notify('started', level);
    await io.execute();
  };
  if (result.status === 'applied') {
    await start();
    return;
  }
  io.notify('requested', level);
  stopWatch = watchRaise({
    level,
    read: io.readLevel,
    onApplied: () => {
      stopWatch = null;
      void start();
    },
  });
}

/** Przerywa czekanie na zgodę (testy, nowa prośba). */
export function stopPlanWatch(): void {
  stopWatch?.();
  stopWatch = null;
}
