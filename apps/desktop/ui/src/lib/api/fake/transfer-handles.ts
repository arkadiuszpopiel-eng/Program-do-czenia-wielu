// Atrapa: jednorazowe uchwyty plików `.alfa` (jak w rdzeniu — UI nigdy nie podaje ścieżki).
// Uchwyt wydaje „dialog” (`transfer.inspect` bez uchwytu), wynik podglądu albo „Przywróć…” z listy
// kopii zapasowych; każde użycie zużywa uchwyt, a wynik podglądu niesie nowy. Czas życia liczony
// zegarem atrapy: kopia z listy — 15 s (UI używa go od razu), podgląd — 10 min (decyzja właściciela).
import type { FakeCore } from './core';

/** Uchwyt kopii z listy (UI woła podgląd od razu). */
export const RESTORE_HANDLE_TTL_MS = 15_000;
/** Uchwyt z wyniku podglądu (hasło, przegląd różnic, import). */
export const REVIEW_HANDLE_TTL_MS = 10 * 60_000;

/** Plik pod uchwytem. */
export interface HandleTarget {
  readonly path: string;
  /** Paczka zaszyfrowana (podgląd bez hasła → prośba o hasło). */
  readonly encrypted: boolean;
}

interface Entry extends HandleTarget {
  readonly expires: number;
}

interface Registry {
  /** Własny licznik (identyfikatory reszty atrapy się nie przesuwają). */
  counter: number;
  readonly entries: Map<string, Entry>;
}

const registries = new WeakMap<FakeCore, Registry>();

function registry(core: FakeCore): Registry {
  let reg = registries.get(core);
  if (!reg) {
    reg = { counter: 0, entries: new Map() };
    registries.set(core, reg);
  }
  return reg;
}

/** Wydaje uchwyt pliku. */
export function issueHandle(core: FakeCore, target: HandleTarget, ttlMs: number): string {
  const reg = registry(core);
  reg.counter++;
  const handle = `fh-${core.scheduler.now().toString(36)}-${reg.counter}`;
  reg.entries.set(handle, { ...target, expires: core.scheduler.now() + ttlMs });
  return handle;
}

/** Zużywa uchwyt; `null` — nieznany, wygasły albo już użyty. */
export function takeHandle(core: FakeCore, handle: string): HandleTarget | null {
  const map = registry(core).entries;
  const entry = map.get(handle);
  map.delete(handle);
  if (!entry || entry.expires < core.scheduler.now()) return null;
  return { path: entry.path, encrypted: entry.encrypted };
}

/** Komunikat jak w rdzeniu (uchwyt wygasł albo został użyty). */
export const HANDLE_EXPIRED = 'Uchwyt pliku wygasł albo został już użyty — wybierz plik ponownie.';
