// Dziennik rozruchu (komenda powłoki `boot_report`, `app-logs` — COMMANDS.md „Polecenia powłoki”).

export type BootVerdict = 'ok' | 'warnings' | 'errors';

export interface BootLine {
  readonly level: 'ERROR' | 'WARN' | 'INFO' | 'DEBUG' | 'TRACE';
  /** Linia jak w pliku dziennika (czas, poziom, źródło, treść) — po redakcji sekretów. */
  readonly text: string;
}

export interface BootReport {
  readonly started_at: string;
  /** Okno rozruchu zamknięte — raport już się nie zmieni. */
  readonly finished: boolean;
  readonly elapsed_ms: number;
  readonly verdict: BootVerdict;
  readonly warnings: number;
  readonly errors: number;
  readonly lines: readonly BootLine[];
  /** Linie pominięte ponad limit 500. */
  readonly dropped: number;
  readonly log_file: string | null;
}
