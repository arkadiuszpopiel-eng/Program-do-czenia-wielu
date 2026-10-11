import { describe, expect, it } from 'vitest';
import { FAKE_HISTORY } from '../../api/fake/api-updates';
import type { ChangeItem, ChangeLogEntry } from '../../api/types-updates';
import { SEVERITIES, countBySeverity, filterHistory, groupBySection } from '../change-history';

const item = (section: string, severity: ChangeItem['severity'], text: string): ChangeItem => ({
  section,
  severity,
  text,
});

const HISTORY: readonly ChangeLogEntry[] = [
  {
    version: '2.0.0',
    date: '2026-10-11',
    summary: 'Nowa wersja.',
    items: [
      item('Głos', 'normal', 'a'),
      item('Kopie', 'critical', 'b'),
      item('Głos', 'serious', 'c'),
    ],
  },
  { version: '1.9.0', date: null, summary: 'Sam wstęp.', items: [] },
  { version: '1.8.0', date: '2026-09-01', summary: '', items: [item('Głos', 'normal', 'd')] },
];

describe('historia zmian', () => {
  it('liczy pozycje każdej wagi', () => {
    expect(countBySeverity(HISTORY)).toEqual({ all: 4, critical: 1, serious: 1, normal: 2 });
    expect(countBySeverity([])).toEqual({ all: 0, critical: 0, serious: 0, normal: 0 });
    expect(SEVERITIES).toEqual(['critical', 'serious', 'normal']);
  });

  it('„wszystkie” zostawia także wpis bez pozycji; konkretna waga go ukrywa', () => {
    expect(filterHistory(HISTORY, 'all')).toBe(HISTORY);
    const normal = filterHistory(HISTORY, 'normal');
    expect(normal.map((e) => e.version)).toEqual(['2.0.0', '1.8.0']);
    expect(normal[0]?.items.map((i) => i.text)).toEqual(['a']);
    const critical = filterHistory(HISTORY, 'critical');
    expect(critical.map((e) => e.version)).toEqual(['2.0.0']);
    expect(critical[0]?.summary).toBe('Nowa wersja.');
  });

  it('grupuje pozycje w sekcje w kolejności pierwszego wystąpienia', () => {
    const groups = groupBySection(HISTORY[0]?.items ?? []);
    expect(groups.map((g) => g.section)).toEqual(['Głos', 'Kopie']);
    expect(groups[0]?.items.map((i) => i.text)).toEqual(['a', 'c']);
    expect(groupBySection([])).toEqual([]);
  });

  it('atrapa ma wszystkie wagi, wpis bez pozycji i wpis bez daty', () => {
    const counts = countBySeverity(FAKE_HISTORY);
    expect(counts.critical).toBeGreaterThan(0);
    expect(counts.serious).toBeGreaterThan(0);
    expect(counts.normal).toBeGreaterThan(0);
    expect(FAKE_HISTORY.some((e) => e.items.length === 0)).toBe(true);
    expect(FAKE_HISTORY.some((e) => e.date === null)).toBe(true);
  });
});
