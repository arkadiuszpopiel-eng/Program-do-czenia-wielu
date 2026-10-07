// Bufor zdarzeń opróżniany najwyżej raz na klatkę (PLAN §14.7: „aktualizacja DOM najwyżej raz na
// klatkę"). Zdarzenia z IPC trafiają do kolejki; jedna funkcja `flush` stosuje całą paczkę naraz.

export interface FrameScheduler {
  request(callback: () => void): number;
  cancel(handle: number): void;
}

export const browserFrames: FrameScheduler = {
  request: (callback) => requestAnimationFrame(() => callback()),
  cancel: (handle) => cancelAnimationFrame(handle),
};

export interface RafBatcherOptions {
  readonly frames?: FrameScheduler;
  /**
   * Gdy okno jest ukryte, przeglądarka wstrzymuje `requestAnimationFrame`. Powyżej tego progu
   * kolejka jest opróżniana od razu, żeby pamięć nie rosła bez końca.
   */
  readonly maxQueue?: number;
}

export class RafBatcher<T> {
  private queue: T[] = [];
  private handle: number | null = null;
  private readonly frames: FrameScheduler;
  private readonly maxQueue: number;
  private disposed = false;

  constructor(
    private readonly flush: (batch: readonly T[]) => void,
    options: RafBatcherOptions = {},
  ) {
    this.frames = options.frames ?? browserFrames;
    this.maxQueue = options.maxQueue ?? 5000;
  }

  get pending(): number {
    return this.queue.length;
  }

  push(...items: readonly T[]): void {
    if (this.disposed || items.length === 0) return;
    this.queue.push(...items);
    if (this.queue.length >= this.maxQueue) {
      this.flushNow();
      return;
    }
    if (this.handle === null) this.handle = this.frames.request(() => this.run());
  }

  /** Natychmiastowe opróżnienie (np. przed przełączeniem sesji). */
  flushNow(): void {
    if (this.handle !== null) {
      this.frames.cancel(this.handle);
      this.handle = null;
    }
    this.run();
  }

  dispose(): void {
    this.disposed = true;
    if (this.handle !== null) this.frames.cancel(this.handle);
    this.handle = null;
    this.queue = [];
  }

  private run(): void {
    this.handle = null;
    if (this.queue.length === 0) return;
    const batch = this.queue;
    this.queue = [];
    this.flush(batch);
  }
}

/** Ręczny zegar klatek do testów: `tick()` wykonuje zaplanowane klatki. */
export class ManualFrames implements FrameScheduler {
  private next = 1;
  private readonly callbacks = new Map<number, () => void>();

  get scheduled(): number {
    return this.callbacks.size;
  }

  request(callback: () => void): number {
    const handle = this.next++;
    this.callbacks.set(handle, callback);
    return handle;
  }

  cancel(handle: number): void {
    this.callbacks.delete(handle);
  }

  tick(): void {
    const due = [...this.callbacks.values()];
    this.callbacks.clear();
    for (const callback of due) callback();
  }
}
