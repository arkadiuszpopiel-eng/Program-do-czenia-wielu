// Atrapa, scenariusz „core-errors”: każda komenda rdzenia — poza startem (`app.bootstrap`,
// `sessions.list`, `settings.schema`) i metodami synchronicznymi — odrzuca błąd w kształcie prawdziwego błędu komendy
// (`CommandError`). Test E2E `e2e/core-errors.spec.ts` przechodzi tak każdy widok i panel: błąd ma
// być pokazany na miejscu (komunikat + „Ponów” albo toast), bez pustej strony i bez nieobsłużonego
// odrzucenia (siatka bezpieczeństwa w `main.ts` zapisuje je w konsoli).
import { CommandError } from '../command-error';

/**
 * Komendy, bez których aplikacja nie wystartuje (ich błąd ma osobny widok „Ponów”), i drzewo stron
 * ustawień (bez niego audyt nie dotarłby do stron; jego błąd pokazuje „Nie udało się wczytać”).
 */
const ALLOWED = new Set(['app.bootstrap', 'sessions.list', 'settings.schema']);
/** Metody synchroniczne (nie wywołują rdzenia). */
const SYNC = new Set(['watchDrag', 'previewUrl']);

export function injectCoreErrors(client: object): void {
  for (const [ns, api] of Object.entries(client)) {
    if (ns === 'core' || !api || typeof api !== 'object') continue;
    const target = api as Record<string, unknown>;
    for (const [name, fn] of Object.entries(target)) {
      if (typeof fn !== 'function' || SYNC.has(name) || ALLOWED.has(`${ns}.${name}`)) continue;
      target[name] = () =>
        Promise.reject(
          new CommandError(
            'internal',
            `Atrapa: rdzeń odrzucił „${ns}.${name}” (scenariusz core-errors).`,
          ),
        );
    }
  }
}

/** Wywołanie komendy w scenariuszu „slow-core” (rejestr dla audytu stanów ładowania). */
export interface CallRecord {
  readonly name: string;
  readonly started: number;
  ended: number | null;
}

declare global {
  interface Window {
    /** Rejestr wywołań atrapy (tylko scenariusz „slow-core”, testy E2E). */
    __alfaCalls?: CallRecord[];
  }
}

/** Zapisuje każde wywołanie komendy (nazwa, start, koniec) w `window.__alfaCalls`. */
export function recordCalls(client: object): void {
  if (typeof window === 'undefined') return;
  const log: CallRecord[] = [];
  window.__alfaCalls = log;
  for (const [ns, api] of Object.entries(client)) {
    if (ns === 'core' || !api || typeof api !== 'object') continue;
    const target = api as Record<string, unknown>;
    for (const [name, fn] of Object.entries(target)) {
      if (typeof fn !== 'function' || SYNC.has(name)) continue;
      const call = fn as (...args: unknown[]) => unknown;
      target[name] = (...args: unknown[]) => {
        const record: CallRecord = {
          name: `${ns}.${name}`,
          started: performance.now(),
          ended: null,
        };
        log.push(record);
        const result = call.apply(api, args);
        if (result instanceof Promise) {
          return result.finally(() => {
            record.ended = performance.now();
          });
        }
        record.ended = performance.now();
        return result;
      };
    }
  }
}
