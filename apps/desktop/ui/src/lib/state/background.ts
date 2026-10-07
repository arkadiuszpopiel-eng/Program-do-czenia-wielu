// Zapisy w tle bez akcji użytkownika (szkic, układ okna) i dziennik błędów w tle — wydzielone
// z AppState. Błąd zapisu w tle nigdy nie przepada po cichu: szkic — toast, układ — konsola.
import { errorText } from '../api/command-error';

/** Błąd w tle bez akcji użytkownika (zapis układu, stan głosu) — tylko do dziennika konsoli. */
export const logFailure =
  (what: string) =>
  (error: unknown): void =>
    console.warn(`Alfa: ${what}: ${errorText(error)}`);

/** Opóźnienie per klucz: kolejne wywołanie przed upływem `ms` zastępuje poprzednie. */
export class Debouncer {
  private readonly pending: Record<string, ReturnType<typeof setTimeout>> = {};

  constructor(private readonly ms: number) {}

  run(key: string, task: () => void): void {
    const existing = this.pending[key];
    if (existing) clearTimeout(existing);
    if (this.ms <= 0) {
      task();
      return;
    }
    this.pending[key] = setTimeout(() => {
      delete this.pending[key];
      task();
    }, this.ms);
  }

  dispose(): void {
    for (const handle of Object.values(this.pending)) clearTimeout(handle);
  }
}

/**
 * Zapis szkicu w tle. Tekst zostaje w polu; błąd zgłasza raz na serię nieudanych zapisów (do
 * pierwszego udanego) — pisanie przy niedziałającym rdzeniu nie zasypuje okna toastami.
 */
export class DraftSaver {
  private failed = false;

  constructor(
    private readonly save: (sessionId: string, text: string) => Promise<void>,
    private readonly report: (error: unknown) => void,
  ) {}

  async run(sessionId: string, text: string): Promise<void> {
    try {
      await this.save(sessionId, text);
      this.failed = false;
    } catch (error) {
      if (!this.failed) this.report(error);
      this.failed = true;
    }
  }
}
