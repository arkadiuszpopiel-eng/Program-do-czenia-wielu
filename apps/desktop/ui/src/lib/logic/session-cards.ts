// Karty rozmów z profilem (P4): widok panelu Sesje zapamiętany na tym komputerze, ruch strzałkami
// po siatce kart i czysty tekst ostatniej wiadomości (atrapa i zabezpieczenie widoku — właściwy
// fragment liczy rdzeń, `app_modules::session_cards`).
import { agentIds, type AgentId } from '@alfa/ui-kit/tokens';

// Zapamiętany widok jest w osobnym module: stan sesji ładuje go przy starcie, resztę — widok kart.
export { readSessionsView, writeSessionsView, type SessionsView } from './sessions-view';

/** Najdłuższy fragment ostatniej wiadomości (znaki) — jak w rdzeniu. */
export const SNIPPET_CHARS = 160;

/**
 * Indeks karty po naciśnięciu klawisza w siatce `columns` kolumn i `count` kart; `null` — klawisz
 * nie przesuwa. Strzałki nie zawijają; Home/End — pierwsza/ostatnia karta.
 */
export function moveInGrid(
  index: number,
  key: string,
  columns: number,
  count: number,
): number | null {
  if (count <= 0) return null;
  const cols = Math.max(1, Math.floor(columns));
  const next =
    key === 'ArrowRight'
      ? index + 1
      : key === 'ArrowLeft'
        ? index - 1
        : key === 'ArrowDown'
          ? index + cols
          : key === 'ArrowUp'
            ? index - cols
            : key === 'Home'
              ? 0
              : key === 'End'
                ? count - 1
                : null;
  if (next === null) return null;
  if (next < 0 || next >= count) return index;
  return next;
}

/**
 * Ruch po kartach podzielonych na grupy (`sizes` — liczba kart w każdej grupie, `at` — indeks w
 * całości). Strzałki działają w obrębie grupy (w bok — w obrębie rzędu); ArrowDown z ostatniego rzędu przechodzi do pierwszej
 * karty następnej grupy, ArrowUp z pierwszego rzędu — do ostatniej karty poprzedniej. Z rzędu nad
 * niepełnym ostatnim ArrowDown trafia na ostatnią kartę grupy. Home/End — pierwsza/ostatnia karta
 * wszystkich grup. `null` — klawisz nie przesuwa.
 */
export function moveInGroups(
  sizes: readonly number[],
  at: number,
  key: string,
  columns: number,
): number | null {
  const total = sizes.reduce((sum, n) => sum + Math.max(0, n), 0);
  if (total <= 0 || at < 0 || at >= total) return null;
  if (key === 'Home') return 0;
  if (key === 'End') return total - 1;
  const cols = Math.max(1, Math.floor(columns));
  let start = 0;
  let group = 0;
  while (group < sizes.length && at >= start + Math.max(0, sizes[group] ?? 0)) {
    start += Math.max(0, sizes[group] ?? 0);
    group++;
  }
  const size = Math.max(0, sizes[group] ?? 0);
  const local = at - start;
  const row = Math.floor(local / cols);
  const lastRow = Math.floor((size - 1) / cols);
  switch (key) {
    case 'ArrowRight':
      return local + 1 < size && (local + 1) % cols !== 0 ? at + 1 : at;
    case 'ArrowLeft':
      return local % cols !== 0 ? at - 1 : at;
    case 'ArrowDown':
      if (row < lastRow) return start + Math.min(local + cols, size - 1);
      return start + size < total ? start + size : at;
    case 'ArrowUp':
      if (row > 0) return at - cols;
      return start > 0 ? start - 1 : at;
    default:
      return null;
  }
}

/** Identyfikatory w partiach po `size` (limit jednego wywołania `sessions_cards`). */
export function inBatches<T>(items: readonly T[], size: number): T[][] {
  const step = Math.max(1, Math.floor(size));
  const out: T[][] = [];
  for (let i = 0; i < items.length; i += step) out.push(items.slice(i, i + step));
  return out;
}

/** Najwięcej kart w jednym wywołaniu — jak `MAX_CARDS` w rdzeniu. */
export const MAX_CARDS = 200;

/** Liczba kolumn siatki: karty w pierwszym rzędzie (ta sama górna krawędź co pierwsza). */
export function columnsOf(tops: readonly number[]): number {
  const first = tops[0];
  if (first === undefined) return 1;
  let n = 0;
  for (const top of tops) {
    if (Math.abs(top - first) > 1) break;
    n++;
  }
  return Math.max(1, n);
}

/** Tylko znane agentki (awatary z ui-kit), bez powtórzeń, do czterech. */
export function knownAgents(ids: readonly string[]): AgentId[] {
  const out: AgentId[] = [];
  for (const id of ids) {
    const agent = agentIds.find((a) => a === id);
    if (agent && !out.includes(agent) && out.length < 4) out.push(agent);
  }
  return out;
}

/** Niewidoczne i zmieniające kierunek tekstu (podmiana kolejności, „pusty” tekst). */
const invisible = (code: number): boolean =>
  (code >= 0x200b && code <= 0x200f) ||
  (code >= 0x202a && code <= 0x202e) ||
  (code >= 0x2060 && code <= 0x2069) ||
  code === 0xfeff;

/** Znaki sterujące (C0 bez końca linii, DEL, C1) → spacja; niewidoczne — usunięte. */
const cleanChars = (text: string): string =>
  Array.from(text, (c) => {
    const code = c.codePointAt(0) ?? 0;
    if (invisible(code)) return '';
    if (code === 0x0a) return c;
    return code < 0x20 || (code >= 0x7f && code < 0xa0) ? ' ' : c;
  }).join('');

/**
 * Czysty tekst z Markdownu: bez znaczników, HTML, znaków sterujących i niewidocznych, w jednej
 * linii, do `max` znaków (dłuższy kończy się wielokropkiem). Wynik wstawiamy jako tekst.
 */
export function plainSnippet(markdown: string, max = SNIPPET_CHARS): string {
  const text = cleanChars(markdown)
    .replace(/```[\s\S]*?(```|$)/g, ' (kod) ')
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/<[^>]*>/g, '')
    .replace(/`([^`]*)`/g, '$1')
    .replace(/^\s{0,3}(#{1,6}|>|[-*+]|\d+[.)])\s+/gm, '')
    .replace(/(\*\*|__|\*|_|~~)(?=\S)([^*_~]*?\S)\1/g, '$2')
    .replace(/\s+/g, ' ')
    .trim();
  const chars = Array.from(text);
  if (chars.length <= max) return text;
  return `${chars
    .slice(0, max - 1)
    .join('')
    .trimEnd()}…`;
}
