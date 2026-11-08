// Logika strony Ustawienia → Diagnostyka komputera: klucze testów, zestawy, formatowanie miar
// i trend wyniku testu w kolejnych przebiegach (porównanie z historią jak w rdzeniu).
import type { CheckKind, CheckRun, Metric, Verdict } from '../api/types-diag';

/** Testy lekkie (bez zapisu na dyski). */
export const QUICK_TESTS = ['integrity', 'cpu_single', 'cpu_multi', 'memory', 'network'] as const;

/** Klucz testu (`disk:C:`) — ten sam co w rdzeniu. */
export function keyOf(kind: CheckKind): string {
  return kind.test === 'disk' ? `disk:${kind.target}` : kind.test;
}

/** Klucze testów dysków z listy dostępnych. */
export const diskKeys = (available: readonly string[]) =>
  available.filter((k) => k.startsWith('disk:'));

/** Klucz i18n nazwy testu i parametr (litera dysku). */
export function testLabelKey(key: string): { key: string; letter?: string } {
  if (key.startsWith('disk:')) return { key: 'diag.test.disk', letter: key.slice(5) };
  return { key: `diag.test.${key}` };
}

/** Klasa koloru wyniku. */
export function verdictClass(v: Verdict): string {
  if (v === 'pass') return 'wk-ok';
  if (v === 'warn') return 'wk-warn';
  if (v === 'fail') return 'wk-error';
  return 'wk-meta';
}

/** Wartość miary z sensowną dokładnością. */
export function fmtValue(m: Pick<Metric, 'value' | 'unit'>): string {
  const v = m.value;
  const digits = Math.abs(v) >= 100 || Number.isInteger(v) ? 0 : Math.abs(v) >= 10 ? 1 : 2;
  return `${v.toLocaleString('pl-PL', { maximumFractionDigits: digits })} ${m.unit}`;
}

/** MB → „12,3 GB”. */
export function gb(mb: number): string {
  return `${(mb / 1024).toLocaleString('pl-PL', { maximumFractionDigits: 1 })} GB`;
}

/** Czas pracy w godzinach i minutach. */
export function uptime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return h ? `${h} h ${m} min` : `${m} min`;
}

/** Wartości miary testu w przebiegach (od najstarszego); `runs` — najnowsze pierwsze. */
export function trend(runs: readonly CheckRun[], test: string, metric: string): number[] {
  const out: number[] = [];
  for (const run of [...runs].reverse()) {
    const r = run.results.find((x) => keyOf(x.kind) === test && x.verdict !== 'skipped');
    const m = r?.metrics.find((x) => x.key === metric);
    if (m) out.push(m.value);
  }
  return out;
}

/** Główna miara testu (do trendu): wynik procesora, zapis dysku, kopiowanie pamięci, połączenie. */
export function mainMetric(test: string): string | null {
  if (test === 'cpu_single' || test === 'cpu_multi') return 'mops';
  if (test === 'memory') return 'copy_gbps';
  if (test === 'network') return 'connect_ms';
  if (test.startsWith('disk:')) return 'seq_write_mbps';
  return null;
}

/** Zmiana ostatniej wartości względem poprzedniej (%), `null` — za mało pomiarów. */
export function change(values: readonly number[]): number | null {
  if (values.length < 2) return null;
  const prev = values[values.length - 2]!;
  const last = values[values.length - 1]!;
  return prev === 0 ? null : ((last - prev) / prev) * 100;
}
