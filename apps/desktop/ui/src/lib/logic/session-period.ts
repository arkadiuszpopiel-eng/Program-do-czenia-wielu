// Archiwum rozmów: filtr okresu listy sesji (P4).
import type { SessionSummary } from '../api/types';

/** Okres na liście rozmów (archiwum): wszystkie, dziś, ostatnie 7 albo 30 dni. */
export type SessionPeriod = 'all' | 'today' | 'week' | 'month';

/** Rozmowy z aktywnością w okresie (po `updated_at`; „dziś” od północy czasu lokalnego). */
export function inPeriod(
  list: readonly SessionSummary[],
  period: SessionPeriod,
  now: number,
): SessionSummary[] {
  if (period === 'all') return [...list];
  const day = 86_400_000;
  const d = new Date(now);
  const midnight = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const since = period === 'today' ? midnight : now - (period === 'week' ? 7 : 30) * day;
  return list.filter((s) => Date.parse(s.updated_at) >= since);
}
