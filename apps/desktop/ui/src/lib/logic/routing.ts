// Dobór modelu według trudności — wersja interfejsu reguł rdzenia (`router-contract::rung`,
// `providers-contract::difficulty`): szczebel z nazwy modelu, uproszczona ocena trudności
// i kolejność prób. Rdzeń decyduje naprawdę; tu korzysta z tego atrapa i podpowiedzi ekranu.
import type { RoutingLevel, RoutingRung, RoutingStrategy } from '../api/types-hub';

export const RUNGS: readonly RoutingRung[] = ['light', 'standard', 'strong'];
export const LEVELS: readonly RoutingLevel[] = ['easy', 'medium', 'hard'];
export const STRATEGIES: readonly RoutingStrategy[] = ['economy', 'balanced', 'quality'];

/** Klasy, w których działa dobór (głos, osadzenia i streszczenia mają stałą kolejność). */
export const DIFFICULTY_CLASSES = ['chat', 'code', 'planning', 'gui_vision'] as const;

const parts = (model: string) => model.toLowerCase().split(/[-_./:]/);

/** Liczba parametrów z nazwy w miliardach (`qwen3-27b` → 27). */
export function paramsBillions(model: string): number | null {
  for (const part of model.toLowerCase().split(/[-_/:]/)) {
    const m = /^(\d+(?:\.\d+)?)b$/.exec(part);
    if (m) return Number(m[1]);
  }
  return null;
}

/** Szczebel z nazwy modelu (jak `rung_by_name` w rdzeniu). */
export function rungByName(model: string): RoutingRung {
  const b = paramsBillions(model);
  if (b !== null) return b <= 9 ? 'light' : b <= 40 ? 'standard' : 'strong';
  const p = parts(model);
  const has = (words: readonly string[]) => p.some((x) => words.includes(x));
  if (has(['nano', 'lite', 'tiny', 'mini', 'flash', 'haiku', 'small', 'fast', 'instant']))
    return 'light';
  if (has(['opus', 'pro', 'ultra', 'max', 'sol', 'fable', 'mythos', 'large', 'reasoner']))
    return 'strong';
  return 'standard';
}

const fold = (text: string) =>
  text
    .toLowerCase()
    .replace(
      /[ąćęłńóśźż]/g,
      (c) => ({ ą: 'a', ć: 'c', ę: 'e', ł: 'l', ń: 'n', ó: 'o', ś: 's', ź: 'z', ż: 'z' })[c] ?? c,
    );

const REASONING = [
  'przeanaliz',
  'analiz',
  'wyjasn',
  'wytlumacz',
  'porownaj',
  'udowodn',
  'uzasadn',
  'zaprojektuj',
  'zaplanuj',
  'zoptymalizuj',
  'refaktor',
  'debug',
  'napraw',
  'zaimplementuj',
  'algorytm',
  'architektur',
  'strategi',
  'explain',
  'compare',
  'prove',
  'design',
  'optimi',
  'refactor',
  'implement',
  'algorithm',
  'architect',
];
const MULTISTEP = ['nastepnie', 'potem', 'najpierw', 'wreszcie', 'finally', 'kolejno'];
const CODE = ['```', 'def ', 'fn ', 'function ', 'class ', 'import ', 'select ', 'async ', '() =>'];

const LABELS: Record<string, string> = {
  code: 'kod',
  reasoning: 'rozumowanie',
  multi: 'wiele kroków',
  lines: 'wiele wierszy',
  questions: 'kilka pytań',
  long: 'długa treść',
};

/** Uproszczona ocena trudności (wynik 0–100 i powody) — dla atrapy. */
export function estimate(text: string): { score: number; reasons: string } {
  const folded = fold(text);
  const words = folded.split(/[^\p{L}\p{N}]+/u).filter(Boolean);
  const stem = (list: readonly string[]) =>
    words.some((w) => list.some((s) => w.startsWith(s)) && !w.startsWith('naprawd'));
  const found: [string, number][] = [];
  if (CODE.some((m) => text.toLowerCase().includes(m))) found.push(['code', 30]);
  if (stem(REASONING) || folded.includes('krok po kroku')) found.push(['reasoning', 32]);
  if (stem(MULTISTEP) || folded.includes('a potem')) found.push(['multi', 26]);
  if (text.split('\n').length > 5) found.push(['lines', 12]);
  if ((text.match(/\?/g) ?? []).length >= 2) found.push(['questions', 8]);
  const chars = [...text].length;
  if (chars >= 300) found.push(['long', 0]);
  const length = Math.floor((Math.min(chars, 600) * 25) / 600);
  const score = Math.min(
    100,
    found.reduce((sum, [, w]) => sum + w, length),
  );
  const reasons = found.length ? found.map(([k]) => LABELS[k]).join(', ') : 'krótka, prosta prośba';
  return { score, reasons };
}

/** Poziom trudności dla wyniku i strategii (progi jak w rdzeniu). */
export function levelOf(score: number, strategy: RoutingStrategy): RoutingLevel {
  const [medium, hard] =
    strategy === 'economy' ? [45, 75] : strategy === 'quality' ? [18, 42] : [28, 58];
  return score >= hard ? 'hard' : score >= medium ? 'medium' : 'easy';
}

export const rungFor = (level: RoutingLevel): RoutingRung =>
  level === 'easy' ? 'light' : level === 'medium' ? 'standard' : 'strong';

/** Kolejność od szczebla `start`: wyższe rosnąco, potem niższe malejąco; remis — lokalne pierwsze. */
export function orderByRung<T extends { rung: RoutingRung; local: boolean }>(
  list: readonly T[],
  start: RoutingRung,
  localFirst: boolean,
): T[] {
  const at = (r: RoutingRung) => RUNGS.indexOf(r);
  const key = (x: T): [number, number, number] => {
    const d = at(x.rung) - at(start);
    return [d >= 0 ? 0 : 1, Math.abs(d), localFirst && !x.local ? 1 : 0];
  };
  return list
    .map((x, i) => ({ x, i, k: key(x) }))
    .sort((a, b) => a.k[0] - b.k[0] || a.k[1] - b.k[1] || a.k[2] - b.k[2] || a.i - b.i)
    .map((e) => e.x);
}
