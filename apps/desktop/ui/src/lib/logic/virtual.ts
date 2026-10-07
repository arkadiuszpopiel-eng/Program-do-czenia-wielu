// Model listy wirtualizowanej o zmiennych wysokościach (PLAN §14.7: renderowane tylko widoczne
// wiadomości ± 1 ekran). Wysokości zmierzone są pamiętane per klucz elementu, więc zmiana
// wariantu/gałęzi albo dopisanie wiadomości nie gubi pomiarów pozostałych.

export interface VisibleRange {
  readonly start: number;
  /** Indeks za ostatnim renderowanym elementem (wyłącznie). */
  readonly end: number;
}

export class VirtualModel {
  private keys: readonly string[] = [];
  private readonly measured = new Map<string, number>();
  private offsets: number[] = [0];
  private dirty = true;

  constructor(private readonly estimate: number = 120) {}

  get count(): number {
    return this.keys.length;
  }

  setKeys(keys: readonly string[]): void {
    this.keys = keys;
    this.dirty = true;
  }

  /** Zwraca `true`, gdy wysokość się zmieniła (trzeba przeliczyć odstępy). */
  setHeight(key: string, height: number): boolean {
    const rounded = Math.max(0, Math.round(height));
    if (this.measured.get(key) === rounded) return false;
    this.measured.set(key, rounded);
    this.dirty = true;
    return true;
  }

  heightOf(index: number): number {
    const key = this.keys[index];
    return key === undefined ? 0 : (this.measured.get(key) ?? this.estimate);
  }

  offsetOf(index: number): number {
    this.ensure();
    const clamped = Math.max(0, Math.min(index, this.keys.length));
    return this.offsets[clamped] ?? 0;
  }

  get totalHeight(): number {
    return this.offsetOf(this.keys.length);
  }

  /** Indeks elementu zawierającego pozycję `y` (wyszukiwanie binarne). */
  indexAt(y: number): number {
    this.ensure();
    let lo = 0;
    let hi = this.keys.length - 1;
    if (hi < 0) return 0;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if ((this.offsets[mid] ?? 0) <= y) lo = mid;
      else hi = mid - 1;
    }
    return lo;
  }

  /** Zakres do renderowania: widoczne elementy ± `overscan` pikseli (domyślnie 1 ekran). */
  range(scrollTop: number, viewport: number, overscan: number = viewport): VisibleRange {
    const n = this.keys.length;
    if (n === 0) return { start: 0, end: 0 };
    const top = Math.max(0, scrollTop - overscan);
    const bottom = scrollTop + viewport + overscan;
    const start = this.indexAt(top);
    let end = this.indexAt(bottom) + 1;
    end = Math.min(n, Math.max(end, start + 1));
    return { start, end };
  }

  /** Usuwa pomiary kluczy, których już nie ma (oszczędność pamięci przy długich sesjach). */
  prune(): void {
    const alive = new Set(this.keys);
    for (const key of this.measured.keys()) if (!alive.has(key)) this.measured.delete(key);
  }

  private ensure(): void {
    if (!this.dirty) return;
    const offsets = new Array<number>(this.keys.length + 1);
    offsets[0] = 0;
    for (let i = 0; i < this.keys.length; i++) {
      offsets[i + 1] = (offsets[i] ?? 0) + this.heightOf(i);
    }
    this.offsets = offsets;
    this.dirty = false;
  }
}

/** Czy lista jest „przyklejona" do dołu (auto-przewijanie aktywne). */
export function isAtBottom(
  scrollTop: number,
  viewport: number,
  total: number,
  slack = 24,
): boolean {
  return total - (scrollTop + viewport) <= slack;
}
