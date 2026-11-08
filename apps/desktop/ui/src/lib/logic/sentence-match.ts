// Dopasowanie czytanego zdania do tekstu wiadomości na ekranie (plan 43). Zdanie mówione różni się
// od wyświetlanego: rdzeń zdejmuje znaczniki, a kod zastępuje opisem „(kod na ekranie)”. Dlatego
// porównujemy same słowa (litery i cyfry, bez wielkości liter) z tolerancją: część słów zdania może
// nie mieć odpowiednika na ekranie. Gdy dopasowanie jest za słabe — brak wyróżnienia (czytanie
// działa dalej).

/** Słowo tekstu: znormalizowana postać i położenie w tekście źródłowym. */
export interface WordAt {
  readonly word: string;
  readonly start: number;
  readonly end: number;
}

/** Zakres dopasowania w tekście źródłowym (indeksy znaków) i numer słowa za nim. */
export interface SentenceSpan {
  readonly start: number;
  readonly end: number;
  readonly nextWord: number;
}

/** Najmniejszy udział słów zdania, które muszą się znaleźć na ekranie. */
export const MIN_MATCH = 0.6;
/** O ile słów ekranu wolno przeskoczyć przy szukaniu kolejnego słowa zdania. */
const LOOKAHEAD = 6;

/** Słowa tekstu (litery i cyfry Unicode) z położeniem. */
export function wordsOf(text: string): WordAt[] {
  const out: WordAt[] = [];
  for (const m of text.matchAll(/[\p{L}\p{N}]+/gu)) {
    out.push({ word: m[0].toLocaleLowerCase('pl'), start: m.index, end: m.index + m[0].length });
  }
  return out;
}

/** Próba dopasowania zdania od słowa ekranu `from`: liczba trafień, pierwsze i ostatnie trafione. */
function align(screen: readonly WordAt[], sentence: readonly string[], from: number) {
  let pos = from;
  let hits = 0;
  let first = -1;
  let last = -1;
  for (const word of sentence) {
    let found = -1;
    for (let k = pos; k < Math.min(screen.length, pos + LOOKAHEAD + 1); k += 1) {
      if (screen[k]?.word === word) {
        found = k;
        break;
      }
    }
    // Słowa spoza ekranu (np. „kod na ekranie”) pomijamy bez przesuwania pozycji.
    if (found < 0) continue;
    hits += 1;
    if (first < 0) first = found;
    last = found;
    pos = found + 1;
  }
  return { hits, first, last };
}

/**
 * Zakres zdania `sentence` w tekście `text`. Wygrywa najlepsze dopasowanie (trafione słowa minus
 * wtrącone słowa ekranu); przy remisie to od słowa `fromWord` (koniec poprzedniego zdania —
 * powtórzone zdanie trafia we właściwe miejsce), potem najbliższe.
 */
export function findSentence(text: string, sentence: string, fromWord = 0): SentenceSpan | null {
  const screen = wordsOf(text);
  const wanted = wordsOf(sentence).map((w) => w.word);
  if (wanted.length === 0 || screen.length === 0) return null;
  const need = Math.max(1, Math.ceil(wanted.length * MIN_MATCH));
  let best: { score: number; rank: number; first: number; last: number } | null = null;
  for (let i = 0; i < screen.length; i += 1) {
    // Zdanie zaczyna się od jednego z pierwszych słów (pierwsze może być „kod” zastąpiony opisem).
    const lead = wanted.slice(0, 3).indexOf(screen[i]?.word ?? '');
    if (lead < 0) continue;
    const { hits, first, last } = align(screen, wanted.slice(lead), i);
    if (hits < need || first < 0) continue;
    const score = 2 * hits - (last - first + 1 - hits);
    const rank = i >= fromWord ? i - fromWord : screen.length + (fromWord - i);
    if (!best || score > best.score || (score === best.score && rank < best.rank)) {
      best = { score, rank, first, last };
    }
  }
  const firstAt = best && screen[best.first];
  const lastAt = best && screen[best.last];
  if (!best || !firstAt || !lastAt) return null;
  return { start: firstAt.start, end: lastAt.end, nextWord: best.last + 1 };
}
