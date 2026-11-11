// Tryb pracy z paska pisania (decyzja właściciela 27.10, od 3.11 na zasadzie Claude Code i Codex —
// plan 50): pięć trybów = poziomy autonomii globalnie, z „Akceptuj edycje” (L2). Podniesienie otwiera kartę w oknie Brokera; zatwierdzone raz
// zostaje także po restarcie (Broker je pamięta). Czekanie na zgodę żyje poza menu — menu można
// zamknąć, a pasek i tak pokaże nowy tryb po zatwierdzeniu.
import type { AutonomyLevel } from '../api/types';
import type { BrokerIntentResult } from '../api/types-hub';
import { watchRaise } from './raise-watch';

/** Tryby w menu — wszystkie poziomy, jak w obu programach (plan 50 §4.1). */
export const WORK_MODES: readonly AutonomyLevel[] = ['L0', 'L1', 'L2', 'L3', 'L4'];

/** Pozycje menu: pięć trybów, a bieżący poziom spoza nich — dopisany, żeby było go widać. */
export function modeItems(current: AutonomyLevel | null): AutonomyLevel[] {
  const items = [...WORK_MODES];
  if (current && !items.includes(current)) {
    items.push(current);
    items.sort();
  }
  return items;
}

export interface ModeRequest {
  readonly request: (level: AutonomyLevel) => Promise<BrokerIntentResult>;
  /** Bieżący poziom globalny po zmianie (`null` — nieznany). */
  readonly readGlobal: () => Promise<AutonomyLevel | null>;
  /** Wynik do pokazania: zastosowano od razu, czeka na okno Brokera, zatwierdzono. */
  readonly notify: (event: 'applied' | 'requested' | 'raised', level: AutonomyLevel) => void;
}

let stopWatch: (() => void) | null = null;
let generation = 0;

/** Zmienia tryb pracy globalnie; nowa prośba kończy czekanie na poprzednią. */
export async function requestMode(level: AutonomyLevel, io: ModeRequest): Promise<void> {
  const mine = ++generation;
  stopWatch?.();
  stopWatch = null;
  const result = await io.request(level);
  if (mine !== generation) return;
  if (result.status === 'applied') {
    io.notify('applied', level);
    return;
  }
  io.notify('requested', level);
  stopWatch = watchRaise({
    level,
    read: io.readGlobal,
    onApplied: () => {
      stopWatch = null;
      io.notify('raised', level);
    },
  });
}

/** Przerywa czekanie (testy, wylogowanie widoku). */
export function stopModeWatch(): void {
  generation += 1;
  stopWatch?.();
  stopWatch = null;
}
