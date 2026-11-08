// Ogłaszanie strumienia dla czytników ekranu: `aria-live` z throttlingiem, na końcu zdania
// (PLAN §14.3 „Dostępność"). Wejście: surowe delty tekstu; wyjście: pełne zdania bez znaczników
// Markdown, bloki kodu zastąpione krótką informacją.

export interface Timers {
  now(): number;
  setTimeout(callback: () => void, ms: number): unknown;
  clearTimeout(handle: unknown): void;
}

export const realTimers: Timers = {
  now: () => performance.now(),
  setTimeout: (callback, ms) => setTimeout(callback, ms),
  clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

export interface AnnouncerOptions {
  /** Minimalny odstęp między ogłoszeniami (ms). */
  readonly minIntervalMs?: number;
  readonly timers?: Timers;
  /** Tekst ogłaszany w miejscu bloku kodu. */
  readonly codeLabel?: string;
}

const SENTENCE_END = /[.!?…]+["'”)\]]*(?=\s)/g;

/** Usuwa znaczniki Markdown, które czytnik przeczytałby dosłownie. */
export function stripMarkdown(text: string): string {
  return text
    .replace(/^\s{0,3}#{1,6}\s+/gm, '')
    .replace(/^\s*[-*+]\s+/gm, '')
    .replace(/\*\*|__|`/g, '')
    .replace(/(^|\s)[*_](\S)/g, '$1$2')
    .replace(/(\S)[*_](?=\s|$|[.,!?;:])/g, '$1')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\s+/g, ' ')
    .trim();
}

export class SentenceAnnouncer {
  private pending = '';
  private carry = '';
  private inCode = false;
  private ready: string[] = [];
  private lastEmit = Number.NEGATIVE_INFINITY;
  private timer: unknown = null;
  private readonly minInterval: number;
  private readonly timers: Timers;
  private readonly codeLabel: string;

  constructor(
    private readonly emit: (text: string) => void,
    options: AnnouncerOptions = {},
  ) {
    this.minInterval = options.minIntervalMs ?? 1200;
    this.timers = options.timers ?? realTimers;
    this.codeLabel = options.codeLabel ?? 'Blok kodu.';
  }

  push(delta: string): void {
    const text = this.carry + delta;
    // Wstrzymaj końcowe „`" — mogą być początkiem ogrodzenia ``` rozciętego między deltami.
    const tail = /`{1,2}$/.exec(text)?.[0] ?? '';
    this.carry = tail;
    const body = tail ? text.slice(0, -tail.length) : text;
    const parts = body.split('```');
    parts.forEach((part, i) => {
      if (i > 0) {
        this.inCode = !this.inCode;
        if (this.inCode) {
          this.extract(true);
          this.ready.push(this.codeLabel);
        }
      }
      if (!this.inCode) this.pending += part;
    });
    this.extract(false);
    this.schedule();
  }

  /** Koniec odpowiedzi: reszta tekstu jest ogłaszana od razu. */
  finish(): void {
    this.carry = '';
    this.extract(true);
    this.inCode = false;
    this.flush();
  }

  reset(): void {
    if (this.timer !== null) this.timers.clearTimeout(this.timer);
    this.timer = null;
    this.pending = '';
    this.carry = '';
    this.inCode = false;
    this.ready = [];
  }

  private extract(all: boolean): void {
    let last = 0;
    for (const match of this.pending.matchAll(SENTENCE_END)) {
      const end = (match.index ?? 0) + match[0].length;
      this.addSentence(this.pending.slice(last, end));
      last = end;
    }
    this.pending = this.pending.slice(last);
    if (all) {
      this.addSentence(this.pending);
      this.pending = '';
    }
  }

  private addSentence(raw: string): void {
    const clean = stripMarkdown(raw);
    if (clean) this.ready.push(clean);
  }

  private schedule(): void {
    if (this.ready.length === 0 || this.timer !== null) return;
    const wait = this.lastEmit + this.minInterval - this.timers.now();
    if (wait <= 0) {
      this.flush();
      return;
    }
    this.timer = this.timers.setTimeout(() => {
      this.timer = null;
      this.flush();
    }, wait);
  }

  private flush(): void {
    if (this.timer !== null) {
      this.timers.clearTimeout(this.timer);
      this.timer = null;
    }
    if (this.ready.length === 0) return;
    const text = this.ready.join(' ');
    this.ready = [];
    this.lastEmit = this.timers.now();
    this.emit(text);
  }
}
