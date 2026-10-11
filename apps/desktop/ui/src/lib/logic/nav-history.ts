// Historia nawigacji „Wstecz / Dalej”: lista odwiedzonych miejsc (rozmowa z sesją albo strona
// Ustawień) z kursorem. Nowe miejsce ucina gałąź „Dalej” — jak w przeglądarce. Najwyżej `cap` pozycji.

export interface Place {
  readonly view: 'chat' | 'settings';
  /** Strona Ustawień (dla rozmowy `null`). */
  readonly page: string | null;
  /** Sesja rozmowy (w Ustawieniach — ta, z której przyszliśmy; wracamy do niej razem ze stroną). */
  readonly session: string | null;
}

export const NAV_HISTORY_CAP = 50;

export function samePlace(a: Place, b: Place): boolean {
  return a.view === b.view && a.page === b.page && a.session === b.session;
}

export class NavHistory {
  #items: Place[] = [];
  #index = -1;
  readonly #cap: number;

  constructor(cap = NAV_HISTORY_CAP) {
    this.#cap = Math.max(2, cap);
  }

  get length(): number {
    return this.#items.length;
  }

  get current(): Place | null {
    return this.#items[this.#index] ?? null;
  }

  get canBack(): boolean {
    return this.#index > 0;
  }

  get canForward(): boolean {
    return this.#index >= 0 && this.#index < this.#items.length - 1;
  }

  /** Zapisuje odwiedzone miejsce; to samo co bieżące — bez zmian. Zwraca `true`, gdy dopisano. */
  record(place: Place): boolean {
    const current = this.current;
    if (current && samePlace(current, place)) return false;
    this.#items = this.#items.slice(0, this.#index + 1);
    this.#items.push(place);
    if (this.#items.length > this.#cap) this.#items = this.#items.slice(-this.#cap);
    this.#index = this.#items.length - 1;
    return true;
  }

  back(): Place | null {
    if (!this.canBack) return null;
    this.#index -= 1;
    return this.current;
  }

  forward(): Place | null {
    if (!this.canForward) return null;
    this.#index += 1;
    return this.current;
  }

  /** Usuwa miejsca, które przestały istnieć (np. usunięta sesja); kursor zostaje na najbliższym. */
  prune(keep: (place: Place) => boolean): void {
    const current = this.current;
    const items = this.#items.filter(keep);
    // Sąsiednie duplikaty po usunięciu scalamy.
    this.#items = items.filter((p, i) => i === 0 || !samePlace(p, items[i - 1] as Place));
    const at = current ? this.#items.findIndex((p) => samePlace(p, current)) : -1;
    this.#index = at >= 0 ? at : this.#items.length - 1;
  }
}
