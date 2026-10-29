// Wyróżnienie czytanego zdania w treści wiadomości (plan 43): CSS Custom Highlight API
// (`CSS.highlights`) — bez zmiany drzewa DOM, więc HTML z Rusta (pulldown-cmark + ammonia) zostaje
// nietknięty. Wygląd: `::highlight(alfa-reading)` w `@alfa/ui-kit/base.css`. Bez obsługi API
// (stara przeglądarka) wyróżnienia nie ma, czytanie działa dalej.
import { findSentence } from '../../logic/sentence-match';
import { reading } from '../../state/reading.svelte';

/** Nazwa wyróżnienia w rejestrze `CSS.highlights`. */
export const READING_HIGHLIGHT = 'alfa-reading';

/** Wiadomość, która ma teraz wyróżnienie (czytana jest najwyżej jedna naraz). */
let owner: HTMLElement | null = null;

function registry(): HighlightRegistry | null {
  return typeof CSS !== 'undefined' && 'highlights' in CSS && typeof Highlight !== 'undefined'
    ? CSS.highlights
    : null;
}

/** Węzły tekstowe treści i ich tekst złączony (bez bloków kodu — rdzeń ich nie czyta). */
function textOf(root: HTMLElement): { nodes: Text[]; starts: number[]; text: string } {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: (n) =>
      n.parentElement?.closest('pre, [aria-hidden="true"]')
        ? NodeFilter.FILTER_REJECT
        : NodeFilter.FILTER_ACCEPT,
  });
  const nodes: Text[] = [];
  const starts: number[] = [];
  let text = '';
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    nodes.push(n as Text);
    starts.push(text.length);
    // Odstęp między węzłami: nagłówek i akapit nie zlewają się w jedno słowo.
    text += `${n.nodeValue ?? ''}\n`;
  }
  return { nodes, starts, text };
}

/** Węzeł i przesunięcie dla indeksu znaku w tekście złączonym. */
function locate(nodes: Text[], starts: number[], at: number): [Text, number] | null {
  for (let i = nodes.length - 1; i >= 0; i -= 1) {
    const start = starts[i] ?? 0;
    const node = nodes[i];
    if (node && at >= start) return [node, Math.min(at - start, node.length)];
  }
  return null;
}

function release(node: HTMLElement): void {
  if (owner !== node) return;
  owner = null;
  registry()?.delete(READING_HIGHLIGHT);
}

/** Akcja na treści wiadomości `turnId`: wyróżnia zdanie, które właśnie słychać. */
export function readingMark(node: HTMLElement, turnId: string) {
  let id = $state(turnId);
  // Słowo za poprzednim zdaniem: kolejne zdanie szukane od tego miejsca.
  let cursor = 0;
  $effect(() => {
    const highlights = registry();
    if (!highlights || reading.turnId !== id) {
      cursor = 0;
      release(node);
      return;
    }
    if (reading.index === 0) cursor = 0;
    const { nodes, starts, text } = textOf(node);
    const span = findSentence(text, reading.text, cursor);
    const from = span && locate(nodes, starts, span.start);
    const to = span && locate(nodes, starts, span.end);
    if (!span || !from || !to) {
      release(node);
      return;
    }
    cursor = span.nextWord;
    const range = document.createRange();
    range.setStart(from[0], from[1]);
    range.setEnd(to[0], to[1]);
    owner = node;
    highlights.set(READING_HIGHLIGHT, new Highlight(range));
  });
  return {
    update(next: string) {
      id = next;
    },
    destroy() {
      release(node);
    },
  };
}

export { voiceOf } from '../../state/reading.svelte';
