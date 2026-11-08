// Wyszukiwanie rozmyte dla palety poleceń (komendy, sesje, ustawienia) i drzewa ustawień.
// Bez zależności; ≤ 16 ms na znak dla kilkuset pozycji (PLAN §14.7). Ignoruje wielkość liter
// i polskie znaki diakrytyczne („pamiec" znajduje „Pamięć").

const DIACRITICS = /[̀-ͯ]/g;

export function normalize(text: string): string {
  return text
    .normalize('NFD')
    .replace(DIACRITICS, '')
    .replace(/ł/g, 'l')
    .replace(/Ł/g, 'l')
    .toLowerCase();
}

const isBoundary = (text: string, i: number): boolean =>
  i === 0 || /[\s\-_/.:,()›▸]/.test(text[i - 1] ?? ' ');

/** Dopasowanie jednego słowa zapytania; 0 = brak, 1 = idealne. Oba argumenty znormalizowane. */
function scoreWord(word: string, target: string): number {
  if (!word) return 1;
  const at = target.indexOf(word);
  if (at === 0) return word.length === target.length ? 1 : 0.95;
  if (at > 0) return isBoundary(target, at) ? 0.85 : 0.7;
  // Podciąg: litery w kolejności, premia za początki słów i ciągłość.
  let ti = 0;
  let hits = 0;
  let boundaryHits = 0;
  let runs = 0;
  let prev = -2;
  for (const ch of word) {
    const found = target.indexOf(ch, ti);
    if (found < 0) return 0;
    if (isBoundary(target, found)) boundaryHits++;
    if (found !== prev + 1) runs++;
    prev = found;
    ti = found + 1;
    hits++;
  }
  const compact = hits / Math.max(hits, runs * 2);
  const starts = boundaryHits / hits;
  return 0.15 + 0.25 * compact + 0.2 * starts;
}

/** Wynik 0..1 dla zapytania względem tekstu (wszystkie słowa zapytania muszą pasować). */
export function fuzzyScore(query: string, target: string): number {
  const q = normalize(query).trim();
  if (!q) return 1;
  const t = normalize(target);
  if (t.includes(q)) return scoreWord(q, t);
  const words = q.split(/\s+/);
  let sum = 0;
  for (const word of words) {
    const s = scoreWord(word, t);
    if (s === 0) return 0;
    sum += s;
  }
  return sum / words.length;
}

/** Najlepszy wynik spośród kilku tekstów (np. etykieta + słowa kluczowe); słowa kluczowe ×0,9. */
export function bestScore(query: string, label: string, keywords: readonly string[] = []): number {
  let best = fuzzyScore(query, label);
  for (const keyword of keywords) best = Math.max(best, fuzzyScore(query, keyword) * 0.9);
  return best;
}

export interface Ranked<T> {
  readonly item: T;
  readonly score: number;
}

export function fuzzyRank<T>(
  items: readonly T[],
  query: string,
  texts: (item: T) => { label: string; keywords?: readonly string[] },
  limit = 50,
): Ranked<T>[] {
  const out: Ranked<T>[] = [];
  for (const item of items) {
    const { label, keywords } = texts(item);
    const score = bestScore(query, label, keywords);
    if (score > 0) out.push({ item, score });
  }
  out.sort((a, b) => b.score - a.score);
  return out.slice(0, limit);
}
