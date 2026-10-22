// Teksty dziennika rozruchu (PL/EN) w module ładowanym po starcie — nie obciążają paczki startowej okna
// głównego (budżet 150 KB gzip, `scripts/bundle-size.mjs`). Parametry `{nazwa}` jak w i18n.
import type { LocalizedText } from '../api/types';

export const BOOT_TEXTS = {
  'toast.ok': { pl: 'Rozruch OK · {s} s', en: 'Startup OK · {s} s' },
  'toast.warnings': {
    pl: 'Rozruch z uwagami · ostrzeżenia: {w} · {s} s',
    en: 'Startup with notes · warnings: {w} · {s} s',
  },
  'toast.errors': {
    pl: 'Rozruch z problemem · błędy: {e}, ostrzeżenia: {w}',
    en: 'Startup with a problem · errors: {e}, warnings: {w}',
  },
  showLog: { pl: 'Pokaż dziennik', en: 'Show log' },
  title: { pl: 'Dziennik rozruchu', en: 'Startup log' },
  intro: {
    pl: 'Wpisy dziennika od startu programu do końca rozruchu (procesy Jądra, Broker, watchdog, moduły, głos). Treści rozmów tu nie ma, sekrety są zamazane. Pełny zapis jest w pliku dziennika.',
    en: 'Log entries from program start until startup ends (kernel processes, Broker, watchdog, modules, voice). No conversation content; secrets are masked. The full record is in the log file.',
  },
  'verdict.ok': { pl: 'Rozruch OK', en: 'Startup OK' },
  'verdict.warnings': { pl: 'Rozruch z uwagami', en: 'Startup with notes' },
  'verdict.errors': { pl: 'Rozruch z problemem', en: 'Startup with a problem' },
  summary: {
    pl: 'czas {s} s · ostrzeżenia: {w} · błędy: {e}',
    en: 'time {s} s · warnings: {w} · errors: {e}',
  },
  running: {
    pl: 'Rozruch trwa — wpisy jeszcze dochodzą.',
    en: 'Startup in progress — entries are still coming in.',
  },
  onlyProblems: { pl: 'Tylko ostrzeżenia i błędy', en: 'Warnings and errors only' },
  copy: { pl: 'Kopiuj dziennik', en: 'Copy log' },
  copied: { pl: 'Dziennik rozruchu skopiowany.', en: 'Startup log copied.' },
  refresh: { pl: 'Odśwież', en: 'Refresh' },
  file: { pl: 'Plik dziennika: {path}', en: 'Log file: {path}' },
  dropped: {
    pl: 'Pominięto {n} dalszych wpisów (limit 500) — są w pliku dziennika.',
    en: '{n} further entries skipped (limit 500) — they are in the log file.',
  },
  empty: { pl: 'Brak wpisów.', en: 'No entries.' },
  loadFailed: {
    pl: 'Nie udało się odczytać dziennika rozruchu: {error}',
    en: 'Could not read the startup log: {error}',
  },
} satisfies Record<string, LocalizedText>;

export type BootTextKey = keyof typeof BOOT_TEXTS;

/** Tekst w języku interfejsu z podstawionymi parametrami (`{s}`, `{w}`…). */
export function bootText(
  locale: 'pl' | 'en',
  key: BootTextKey,
  params: Readonly<Record<string, string | number>> = {},
): string {
  return BOOT_TEXTS[key][locale].replace(/\{(\w+)\}/g, (all, name: string) =>
    name in params ? String(params[name]) : all,
  );
}
