// Ustawienia → Polecenia bez modelu (komendy `quick_commands_*`): własne polecenia, katalog
// wbudowanych, tester.

export type QuickMatchMode = 'exact' | 'starts' | 'contains' | 'regex';

/** Własne polecenie: frazy → stała odpowiedź. */
export interface CustomCommand {
  /** Pusty przy dodawaniu — nadaje rdzeń. */
  readonly id: string;
  readonly name: string;
  readonly phrases: readonly string[];
  readonly mode: QuickMatchMode;
  /** Odpowiedź (Markdown) z `{reszta}` i nazwanymi grupami wyrażenia. */
  readonly reply: string;
  readonly enabled: boolean;
  /** Wyższy wygrywa (−10…10). */
  readonly priority: number;
}

export interface BuiltinCommand {
  readonly title: string;
  readonly examples: readonly string[];
}

export interface QuickCommandsView {
  readonly commands: readonly CustomCommand[];
  readonly builtins: readonly BuiltinCommand[];
  /** Polecenia bez modelu włączone (`sessions.local_commands`). */
  readonly enabled: boolean;
}

export type QuickSource = 'custom' | 'builtin' | 'quiz' | 'model' | 'disabled';

/** Wynik testera: kto odpowie na wiadomość. */
export interface QuickTest {
  readonly source: QuickSource;
  readonly name: string | null;
  readonly reply: string | null;
}
