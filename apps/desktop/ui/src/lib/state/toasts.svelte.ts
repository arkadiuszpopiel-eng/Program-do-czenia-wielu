// Małe toasty w prawym dolnym rogu (PLAN §14.8). Czas życia wstrzymywany, gdy użytkownik
// najedzie lub ustawi fokus na toaście (WCAG 2.2.1 — regulowany limit czasu).
import type { ToastKind } from '@alfa/ui-kit';

export interface ToastItem {
  readonly id: number;
  readonly kind: ToastKind;
  readonly message: string;
  readonly actionLabel?: string;
  readonly onAction?: () => void;
  readonly timeoutMs: number;
}

export interface ToastTimers {
  setTimeout(callback: () => void, ms: number): unknown;
  clearTimeout(handle: unknown): void;
}

const browserTimers: ToastTimers = {
  setTimeout: (callback, ms) => setTimeout(callback, ms),
  clearTimeout: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

export class ToastState {
  items = $state<ToastItem[]>([]);
  private nextId = 1;
  private readonly timers: Record<number, unknown> = {};

  constructor(private readonly clock: ToastTimers = browserTimers) {}

  show(toast: Omit<ToastItem, 'id' | 'timeoutMs'> & { timeoutMs?: number }): number {
    const id = this.nextId++;
    const item: ToastItem = { timeoutMs: 5000, ...toast, id };
    this.items = [...this.items.slice(-3), item];
    this.arm(item);
    return id;
  }

  dismiss(id: number): void {
    this.pause(id);
    this.items = this.items.filter((t) => t.id !== id);
  }

  /** Wstrzymanie (hover / fokus) i wznowienie odliczania. */
  pause(id: number): void {
    const handle = this.timers[id];
    if (handle !== undefined) this.clock.clearTimeout(handle);
    delete this.timers[id];
  }

  resume(id: number): void {
    const item = this.items.find((t) => t.id === id);
    if (item && !(id in this.timers)) this.arm(item);
  }

  act(id: number): void {
    const item = this.items.find((t) => t.id === id);
    this.dismiss(id);
    item?.onAction?.();
  }

  private arm(item: ToastItem): void {
    if (item.timeoutMs <= 0) return;
    this.timers[item.id] = this.clock.setTimeout(() => this.dismiss(item.id), item.timeoutMs);
  }
}
