// Katalog modeli (Ustawienia → Modele lokalne i silniki): wyszukiwanie po nazwie, sortowanie
// i podsumowanie miejsca na dysku. Filtry rodzaju i stanu są w `models.ts`.
import type { ModelItem } from '../api/types-models';
import { foldText } from './shortcut-search';

export type SortOrder = 'catalog' | 'name' | 'size_desc' | 'size_asc';

/** Kolejności listy: jak w katalogu (domyślna), po nazwie, od największych, od najmniejszych. */
export const SORT_ORDERS: readonly SortOrder[] = ['catalog', 'name', 'size_desc', 'size_asc'];

/**
 * Pozycje pasujące do zapytania: każde słowo musi wystąpić w nazwie, identyfikatorze, licencji
 * albo źródle. Bez rozróżniania wielkości liter i polskich znaków; puste zapytanie nie filtruje.
 */
export function searchItems(items: readonly ModelItem[], query: string): readonly ModelItem[] {
  const words = foldText(query).split(' ').filter(Boolean);
  if (words.length === 0) return items;
  return items.filter((item) => {
    const hay = foldText([item.name, item.id, item.license, item.source].join(' '));
    return words.every((word) => hay.includes(word));
  });
}

/** Lista w wybranej kolejności (kopia; „catalog” zostawia kolejność rdzenia). */
export function sortItems(items: readonly ModelItem[], order: SortOrder): readonly ModelItem[] {
  if (order === 'catalog') return items;
  const byName = (a: ModelItem, b: ModelItem) => a.name.localeCompare(b.name, 'pl');
  const sorted = [...items];
  if (order === 'name') return sorted.sort(byName);
  const sign = order === 'size_desc' ? -1 : 1;
  return sorted.sort((a, b) => sign * (a.size_bytes - b.size_bytes) || byName(a, b));
}

export interface DiskUsage {
  /** Pozycje, których pliki są na dysku (zainstalowane albo wgrane ręcznie). */
  readonly installedCount: number;
  readonly installedBytes: number;
  /** Pozycje do pobrania w aplikacji, których jeszcze nie ma. */
  readonly availableCount: number;
  readonly availableBytes: number;
}

/** Ile miejsca zajmują pozycje na dysku i ile zajęłyby te, które można jeszcze pobrać. */
export function diskUsage(items: readonly ModelItem[]): DiskUsage {
  const usage = { installedCount: 0, installedBytes: 0, availableCount: 0, availableBytes: 0 };
  for (const item of items) {
    if (item.state === 'installed' || item.state === 'external') {
      usage.installedCount += 1;
      usage.installedBytes += item.size_bytes;
    } else if (item.state === 'missing' && item.downloadable) {
      usage.availableCount += 1;
      usage.availableBytes += item.size_bytes;
    }
  }
  return usage;
}
