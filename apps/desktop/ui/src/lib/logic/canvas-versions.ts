// Historia wersji artefaktu w kanwie (C2.8): kolejne odpowiedzi na ścieżce rozmowy z artefaktem
// tego samego rodzaju i o tym samym tytule to wersje jednego artefaktu. Historia jest tylko do
// odczytu (rozmowa append-only) — wynika z tur, niczego nie zapisuje. Logika bez DOM.
import { detectArtifacts, type CanvasItem } from './canvas';

type TurnLike = Parameters<typeof detectArtifacts>[0];

/** Klucz „ten sam artefakt”: rodzaj i tytuł (bez wielkości liter i nadmiaru spacji); bez tytułu — brak historii. */
export function versionKey(item: Pick<CanvasItem, 'kind' | 'title' | 'origin'>): string | null {
  if (item.origin.type === 'file' || !item.title) return null;
  return `${item.kind}:${item.title.toLocaleLowerCase('pl').replace(/\s+/g, ' ').trim()}`;
}

/**
 * Wersje artefaktu na widocznej ścieżce rozmowy, od najstarszej. W jednej odpowiedzi liczy się
 * pierwszy pasujący artefakt (dwa bloki o tym samym tytule w jednej turze to nie dwie wersje).
 * Artefakt bez tytułu albo z pliku — tylko on sam.
 */
export function artifactVersions(path: readonly TurnLike[], item: CanvasItem): CanvasItem[] {
  const key = versionKey(item);
  if (!key) return [item];
  const found: CanvasItem[] = [];
  for (const turn of path) {
    const artifacts = detectArtifacts(turn);
    const match =
      artifacts.find((a) => a.key === item.key) ?? artifacts.find((a) => versionKey(a) === key);
    if (match) found.push(match.key === item.key ? item : match);
  }
  return found.some((v) => v.key === item.key) ? found : [item];
}

export type DiffOp = 'same' | 'add' | 'del';

export interface DiffLine {
  readonly op: DiffOp;
  readonly text: string;
}

/** Fragment porównania: zmienione wiersze z kontekstem albo pominięty ciąg bez zmian. */
export type DiffChunk =
  | { readonly type: 'lines'; readonly lines: readonly DiffLine[] }
  | { readonly type: 'skip'; readonly count: number };

export interface LineDiff {
  readonly added: number;
  readonly removed: number;
  readonly chunks: readonly DiffChunk[];
}

/** Najwięcej wierszy jednej wersji do porównania (tablica LCS rośnie z iloczynem długości). */
export const DIFF_MAX_LINES = 2000;
/** Wiersze bez zmian pokazywane wokół zmiany. */
export const DIFF_CONTEXT = 3;

const lines = (text: string): string[] => text.replace(/\r\n?/g, '\n').split('\n');

/** Porównanie wierszy (najdłuższy wspólny podciąg); `null` — wersje za duże do porównania. */
export function diffLines(before: string, after: string): DiffLine[] | null {
  const a = lines(before);
  const b = lines(after);
  if (a.length > DIFF_MAX_LINES || b.length > DIFF_MAX_LINES) return null;
  const width = b.length + 1;
  const table = new Uint16Array((a.length + 1) * width);
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      table[i * width + j] =
        a[i] === b[j]
          ? (table[(i + 1) * width + j + 1] ?? 0) + 1
          : Math.max(table[(i + 1) * width + j] ?? 0, table[i * width + j + 1] ?? 0);
    }
  }
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < a.length || j < b.length) {
    const left = a[i];
    const right = b[j];
    if (i < a.length && j < b.length && left === right) {
      out.push({ op: 'same', text: left ?? '' });
      i++;
      j++;
    } else if (
      i < a.length &&
      (j >= b.length || (table[(i + 1) * width + j] ?? 0) >= (table[i * width + j + 1] ?? 0))
    ) {
      out.push({ op: 'del', text: left ?? '' });
      i++;
    } else {
      out.push({ op: 'add', text: right ?? '' });
      j++;
    }
  }
  return out;
}

/** Zmiany z kontekstem `DIFF_CONTEXT` wierszy; dłuższe ciągi bez zmian zwinięte w „pominięto N”. */
export function diffChunks(diff: readonly DiffLine[]): LineDiff {
  const keep = diff.map(() => false);
  diff.forEach((line, index) => {
    if (line.op === 'same') return;
    const from = Math.max(0, index - DIFF_CONTEXT);
    const to = Math.min(diff.length - 1, index + DIFF_CONTEXT);
    for (let k = from; k <= to; k++) keep[k] = true;
  });
  const chunks: DiffChunk[] = [];
  let current: DiffLine[] = [];
  let skipped = 0;
  const flush = () => {
    if (current.length) chunks.push({ type: 'lines', lines: current });
    current = [];
  };
  diff.forEach((line, index) => {
    if (keep[index]) {
      if (skipped) chunks.push({ type: 'skip', count: skipped });
      skipped = 0;
      current.push(line);
    } else {
      flush();
      skipped++;
    }
  });
  flush();
  if (skipped) chunks.push({ type: 'skip', count: skipped });
  return {
    added: diff.filter((l) => l.op === 'add').length,
    removed: diff.filter((l) => l.op === 'del').length,
    chunks,
  };
}
