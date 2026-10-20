// Karty rozmów z profilem (P4): widok panelu Sesje zapamiętany na tym komputerze, ruch strzałkami
// po siatce kart i czysty tekst ostatniej wiadomości (atrapa i zabezpieczenie widoku — właściwy
// fragment liczy rdzeń, `app_modules::session_cards`).
import { agentIds, type AgentId } from '@alfa/ui-kit';

export type SessionsView = 'list' | 'cards';

const VIEW_KEY = 'alfa.sessions.view';
/** Najdłuższy fragment ostatniej wiadomości (znaki) — jak w rdzeniu. */
export const SNIPPET_CHARS = 160;

/** Widok z poprzedniego uruchomienia (pamięć tego komputera); brak albo błąd — lista. */
export function readSessionsView(storage: Pick<Storage, 'getItem'> | null): SessionsView {
  try {
    return storage?.getItem(VIEW_KEY) === 'cards' ? 'cards' : 'list';
  } catch {
    return 'list';
  }
}

export function writeSessionsView(storage: Pick<Storage, 'setItem'> | null, view: SessionsView) {
  try {
    storage?.setItem(VIEW_KEY, view);
  } catch {
    // Pamięć przeglądarki niedostępna — widok działa do zamknięcia okna.
  }
}

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
