// Atrapa: wyrażenia cron (5 pól: minuta godzina dzień miesiąc dzień-tygodnia; `*`, listy,
// zakresy, kroki) — najbliższe uruchomienia w czasie lokalnym (rdzeń liczy w Europe/Warsaw z DST).
const RANGES: readonly (readonly [number, number])[] = [
  [0, 59],
  [0, 23],
  [1, 31],
  [1, 12],
  [0, 7],
];

function field(spec: string, [min, max]: readonly [number, number]): Set<number> | null {
  const out = new Set<number>();
  for (const part of spec.split(',')) {
    const [range = '', stepText] = part.split('/');
    const step = stepText === undefined ? 1 : Number(stepText);
    if (!Number.isInteger(step) || step < 1) return null;
    let lo = min;
    let hi = max;
    if (range !== '*') {
      const [a = '', b] = range.split('-');
      lo = Number(a);
      hi = b === undefined ? (stepText === undefined ? lo : max) : Number(b);
    }
    if (!Number.isInteger(lo) || !Number.isInteger(hi) || lo < min || hi > max || lo > hi) {
      return null;
    }
    for (let v = lo; v <= hi; v += step) out.add(v === 7 && max === 7 ? 0 : v);
  }
  return out;
}

/** Najbliższe uruchomienia albo opis błędu. */
export function cronNext(
  expr: string,
  from: number,
  count: number,
): { readonly next: number[] } | { readonly error: string } {
  const parts = expr.trim().split(/\s+/);
  if (parts.length !== 5)
    return { error: 'Wyrażenie musi mieć 5 pól: min godz dzień mies dzień-tyg.' };
  const sets = parts.map((p, i) => field(p, RANGES[i] ?? [0, 0]));
  if (sets.some((s) => s === null)) return { error: 'Nieprawidłowe pole wyrażenia cron.' };
  const [min, hour, dom, mon, dow] = sets as Set<number>[];
  const next: number[] = [];
  const t = new Date(from);
  t.setSeconds(0, 0);
  t.setMinutes(t.getMinutes() + 1);
  // Najwyżej rok naprzód: dni i godziny spoza zbiorów pomijane w całości.
  const end = from + 366 * 24 * 3_600_000;
  while (next.length < count && t.getTime() <= end) {
    const dayOk = mon?.has(t.getMonth() + 1) && dom?.has(t.getDate()) && dow?.has(t.getDay());
    if (!dayOk) {
      t.setHours(24, 0, 0, 0);
    } else if (!hour?.has(t.getHours())) {
      t.setHours(t.getHours() + 1, 0, 0, 0);
    } else {
      if (min?.has(t.getMinutes())) next.push(t.getTime());
      t.setMinutes(t.getMinutes() + 1);
    }
  }
  return { next };
}
