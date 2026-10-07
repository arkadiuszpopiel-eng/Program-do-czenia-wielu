// Logika composera bez DOM: historia wysłanych (Ctrl+↑/↓), wykrywanie `@agentka` i `/komenda`
// przy kursorze, wstawianie podpowiedzi.

export class SentHistory {
  private items: string[] = [];
  private cursor = -1;
  private stash = '';

  constructor(private readonly capacity = 50) {}

  get size(): number {
    return this.items.length;
  }

  push(text: string): void {
    const trimmed = text.trim();
    if (!trimmed) return;
    if (this.items[this.items.length - 1] !== trimmed) this.items.push(trimmed);
    if (this.items.length > this.capacity) this.items.shift();
    this.reset();
  }

  /** Ctrl+↑: poprzednia wysłana wiadomość (bieżący szkic zapamiętany). */
  prev(current: string): string | null {
    if (this.items.length === 0) return null;
    if (this.cursor === -1) {
      this.stash = current;
      this.cursor = this.items.length;
    }
    if (this.cursor === 0) return this.items[0] ?? null;
    this.cursor--;
    return this.items[this.cursor] ?? null;
  }

  /** Ctrl+↓: następna; za ostatnią wraca szkic sprzed przeglądania. */
  next(): string | null {
    if (this.cursor === -1) return null;
    this.cursor++;
    if (this.cursor >= this.items.length) {
      const stash = this.stash;
      this.reset();
      return stash;
    }
    return this.items[this.cursor] ?? null;
  }

  reset(): void {
    this.cursor = -1;
    this.stash = '';
  }
}

export interface Trigger {
  readonly kind: '@' | '/';
  readonly query: string;
  /** Pozycja znaku wyzwalacza. */
  readonly start: number;
  /** Pozycja kursora (koniec zapytania). */
  readonly end: number;
}

/**
 * `@` na początku słowa — adresowanie agentki; `/` tylko na początku wiadomości — komenda.
 * Zwraca `null`, gdy kursor nie stoi w takim tokenie.
 */
export function findTrigger(text: string, caret: number): Trigger | null {
  const before = text.slice(0, caret);
  const match = /(^|\s)([@/])([\p{L}\p{N}_-]*)$/u.exec(before);
  if (!match) return null;
  const kind = match[2] as '@' | '/';
  const start = before.length - (match[3]?.length ?? 0) - 1;
  if (kind === '/' && before.slice(0, start).trim() !== '') return null;
  return { kind, query: match[3] ?? '', start, end: caret };
}

/** Wstawia podpowiedź w miejsce tokenu; zwraca nowy tekst i pozycję kursora. */
export function applyCompletion(
  text: string,
  trigger: Trigger,
  replacement: string,
): { text: string; caret: number } {
  const insert = `${trigger.kind}${replacement} `;
  const after = text.slice(trigger.end).replace(/^\S*/, '');
  const next = text.slice(0, trigger.start) + insert + after.replace(/^ /, '');
  return { text: next, caret: trigger.start + insert.length };
}

/** Adresatka wiadomości z pierwszego `@imię` (zwrot po imieniu zawsze wygrywa, PLAN §9.2). */
export function addressedAgent<T extends string>(
  text: string,
  agents: readonly { id: T; name: string }[],
): T | null {
  const match = /(?:^|\s)@([\p{L}]+)/u.exec(text);
  if (!match?.[1]) return null;
  const name = match[1].toLowerCase();
  return agents.find((a) => a.name.toLowerCase() === name || a.id === name)?.id ?? null;
}
