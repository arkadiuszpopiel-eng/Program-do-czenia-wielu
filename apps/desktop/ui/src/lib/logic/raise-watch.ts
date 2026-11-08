// Czekanie na podniesienie poziomu autonomii zatwierdzone w oknie Brokera. Strona Uprawnień wysyła
// prośbę (`opened_broker`), a zgodę daje osobny proces (Broker-UI) — nic nie odświeża strony, gdy
// Broker przyjmie nowy poziom (zgłoszenie 2026-10-21: L4 zatwierdzone, strona dalej pokazywała L3).
// Sprawdzamy poziom co chwilę, aż będzie taki jak w prośbie — najdłużej tyle, ile żyje karta zgody.
import type { AutonomyLevel } from '../api/types';

/** Co ile sprawdzać poziom (ms). */
export const RAISE_POLL_MS = 2_000;
/** Jak długo czekać: karta w oknie Brokera wygasa po 10 min. */
export const RAISE_WATCH_MS = 10 * 60_000;

export interface RaiseWatch {
  /** Poziom, o który poproszono. */
  readonly level: AutonomyLevel;
  /** Bieżący poziom w zakresie prośby (globalny albo sesji); `null` — nieznany. */
  readonly read: () => Promise<AutonomyLevel | null>;
  /** Broker przyjął poziom z prośby. */
  readonly onApplied: () => void;
  readonly everyMs?: number;
  readonly forMs?: number;
}

/** Zaczyna czekanie; zwraca funkcję, która je przerywa (np. przy opuszczeniu strony). */
export function watchRaise(watch: RaiseWatch): () => void {
  const every = watch.everyMs ?? RAISE_POLL_MS;
  let left = Math.ceil((watch.forMs ?? RAISE_WATCH_MS) / every);
  let busy = false;
  // Odczyt w locie po przerwaniu (nowa prośba, opuszczenie strony) nie może już niczego zgłosić.
  let stopped = false;
  const timer = setInterval(() => {
    if (busy || stopped) return;
    if (left <= 0) {
      clearInterval(timer);
      return;
    }
    left -= 1;
    busy = true;
    watch
      .read()
      .then((level) => {
        if (!stopped && level === watch.level) {
          clearInterval(timer);
          left = 0;
          watch.onApplied();
        }
      })
      // Chwilowy błąd odczytu — następna próba za `every` ms.
      .catch(() => undefined)
      .finally(() => {
        busy = false;
      });
  }, every);
  return () => {
    stopped = true;
    clearInterval(timer);
  };
}
