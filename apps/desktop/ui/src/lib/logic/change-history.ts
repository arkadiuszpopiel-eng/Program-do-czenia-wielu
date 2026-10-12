// Historia zmian (Ustawienia → Aktualizacje): liczenie pozycji według wagi, filtr i grupowanie
// pozycji wpisu w sekcje. Dane przychodzą z rdzenia (`updates_history`), który czyta dziennik
// zmian wbudowany w program.
import type { ChangeItem, ChangeLogEntry, ChangeSeverity } from '../api/types-updates';

/** Wagi w kolejności od najważniejszej. */
export const SEVERITIES: readonly ChangeSeverity[] = ['critical', 'serious', 'normal'];

export type SeverityFilter = ChangeSeverity | 'all';

export type SeverityCounts = Readonly<Record<ChangeSeverity | 'all', number>>;

/** Liczba pozycji każdej wagi we wpisach. */
export function countBySeverity(entries: readonly ChangeLogEntry[]): SeverityCounts {
  const counts = { all: 0, critical: 0, serious: 0, normal: 0 };
  for (const entry of entries) {
    for (const item of entry.items) {
      counts.all += 1;
      counts[item.severity] += 1;
    }
  }
  return counts;
}

/**
 * Wpisy z pozycjami wybranej wagi. Przy „wszystkie” wpis bez pozycji (samo zdanie wstępu) zostaje;
 * przy konkretnej wadze znika wpis, który takich pozycji nie ma.
 */
export function filterHistory(
  entries: readonly ChangeLogEntry[],
  filter: SeverityFilter,
): readonly ChangeLogEntry[] {
  if (filter === 'all') return entries;
  return entries
    .map((entry) => ({ ...entry, items: entry.items.filter((item) => item.severity === filter) }))
    .filter((entry) => entry.items.length > 0);
}

export interface SectionGroup {
  readonly section: string;
  readonly items: readonly ChangeItem[];
}

/** Pozycje wpisu w sekcjach, w kolejności pierwszego wystąpienia sekcji. */
export function groupBySection(items: readonly ChangeItem[]): readonly SectionGroup[] {
  const groups: { section: string; items: ChangeItem[] }[] = [];
  for (const item of items) {
    const group = groups.find((g) => g.section === item.section);
    if (group) group.items.push(item);
    else groups.push({ section: item.section, items: [item] });
  }
  return groups;
}

/** Ile najnowszych wersji pokazuje główny widok historii; starsze są w osobnej zakładce. */
export const RECENT_VERSIONS = 10;

export type HistoryTab = 'recent' | 'older';

export interface HistorySplit {
  readonly recent: readonly ChangeLogEntry[];
  readonly older: readonly ChangeLogEntry[];
}

/** Podział historii (od najnowszej) na ostatnie wersje i starsze. */
export function splitHistory(
  entries: readonly ChangeLogEntry[],
  limit: number = RECENT_VERSIONS,
): HistorySplit {
  const cut = Math.max(0, Math.floor(limit));
  return { recent: entries.slice(0, cut), older: entries.slice(cut) };
}

/** Wpis dziennika dla wersji (numer bez początkowego „v”); `null` — dziennik jej nie zna. */
export function entryFor(
  entries: readonly ChangeLogEntry[],
  version: string,
): ChangeLogEntry | null {
  const wanted = version.trim().replace(/^v/i, '');
  return entries.find((entry) => entry.version === wanted) ?? null;
}
