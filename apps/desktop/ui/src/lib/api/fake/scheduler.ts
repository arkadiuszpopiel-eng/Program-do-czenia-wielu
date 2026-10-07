// Zegar atrapy: prawdziwy (dev, Storybook) albo wirtualny (testy — deterministyczny czas).

export interface Scheduler {
  /** Czas epoki w ms (daty tur, oś czasu). */
  now(): number;
  setTimeout(callback: () => void, ms: number): number;
  clearTimeout(handle: number): void;
}

export const realScheduler: Scheduler = {
  now: () => Date.now(),
  setTimeout: (callback, ms) => window.setTimeout(callback, ms),
  clearTimeout: (handle) => window.clearTimeout(handle),
};

interface Task {
  readonly id: number;
  readonly at: number;
  readonly callback: () => void;
}

/** Wirtualny zegar: `advance(ms)` wykonuje zaplanowane zadania w kolejności czasu. */
export class VirtualScheduler implements Scheduler {
  private time: number;
  private nextId = 1;
  private tasks: Task[] = [];

  constructor(start: number = Date.UTC(2026, 8, 30, 8, 45)) {
    this.time = start;
  }

  now(): number {
    return this.time;
  }

  get pending(): number {
    return this.tasks.length;
  }

  setTimeout(callback: () => void, ms: number): number {
    const id = this.nextId++;
    this.tasks.push({ id, at: this.time + Math.max(0, ms), callback });
    return id;
  }

  clearTimeout(handle: number): void {
    this.tasks = this.tasks.filter((task) => task.id !== handle);
  }

  advance(ms: number): void {
    const target = this.time + ms;
    for (;;) {
      const due = this.tasks
        .filter((task) => task.at <= target)
        .sort((a, b) => a.at - b.at || a.id - b.id)[0];
      if (!due) break;
      this.tasks = this.tasks.filter((task) => task.id !== due.id);
      this.time = due.at;
      due.callback();
    }
    this.time = target;
  }

  /** Wykonuje wszystko (z limitem bezpieczeństwa na nieskończone pętle). */
  runAll(limitMs = 3_600_000): void {
    this.advance(limitMs);
  }
}
