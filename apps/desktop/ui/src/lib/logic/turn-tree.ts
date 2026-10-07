// Drzewo gałęzi rozmowy (ADR 0006): dziennik tur jest append-only, a widoczna rozmowa to
// projekcja — ścieżka od korzenia, na każdym rozwidleniu wybrany wariant (domyślnie najnowszy).
// Funkcje nie modyfikują istniejących tur; dodają jedynie nowe węzły i zmieniają wybór wariantu.
import type { Turn } from '../api/types';

/** Klucz rodzica dla tur bez rodzica (korzeń rozmowy). */
export const ROOT_KEY = '$root';

export interface TurnTree {
  readonly turns: Record<string, Turn>;
  /** Dzieci per rodzic w kolejności utworzenia. */
  readonly children: Record<string, string[]>;
  /** Wybrany wariant per rodzic; brak wpisu = najnowszy. */
  readonly selection: Record<string, string>;
}

export const parentKey = (turn: Pick<Turn, 'parent_id'>): string => turn.parent_id ?? ROOT_KEY;

const byCreation = (a: Turn, b: Turn): number =>
  a.created_at === b.created_at ? (a.id < b.id ? -1 : 1) : a.created_at < b.created_at ? -1 : 1;

export function emptyTree(): TurnTree {
  return { turns: {}, children: {}, selection: {} };
}

export function buildTree(turns: readonly Turn[]): TurnTree {
  const tree = emptyTree();
  for (const turn of [...turns].sort(byCreation)) addTurn(tree, turn);
  return tree;
}

/** Dodaje turę (idempotentnie). Zwraca `false`, jeśli tura już istniała. */
export function addTurn(tree: TurnTree, turn: Turn): boolean {
  if (tree.turns[turn.id]) return false;
  tree.turns[turn.id] = turn;
  const key = parentKey(turn);
  // Zawsze przez obiekt drzewa (nie przez lokalną referencję nowej tablicy) — przy stanie Svelte
  // przypisana tablica jest opakowywana w proxy, a mutacja surowej tablicy nie byłaby widoczna.
  const list = tree.children[key];
  if (list) list.push(turn.id);
  else tree.children[key] = [turn.id];
  return true;
}

function selectedChild(tree: TurnTree, key: string): string | undefined {
  const list = tree.children[key];
  if (!list || list.length === 0) return undefined;
  const chosen = tree.selection[key];
  return chosen && list.includes(chosen) ? chosen : list[list.length - 1];
}

/** Widoczna ścieżka rozmowy od korzenia do liścia. */
export function projectPath(tree: TurnTree): Turn[] {
  const path: Turn[] = [];
  const seen = new Set<string>();
  let id = selectedChild(tree, ROOT_KEY);
  while (id !== undefined && !seen.has(id)) {
    seen.add(id);
    const turn = tree.turns[id];
    if (!turn) break;
    path.push(turn);
    id = selectedChild(tree, turn.id);
  }
  return path;
}

export interface SiblingInfo {
  /** Numer wariantu od 1. */
  readonly index: number;
  readonly total: number;
}

export function siblingInfo(tree: TurnTree, turn: Turn): SiblingInfo {
  const list = tree.children[parentKey(turn)] ?? [turn.id];
  return { index: Math.max(0, list.indexOf(turn.id)) + 1, total: list.length };
}

/** Przełącza wariant `‹ ›` o `delta` (z zawijaniem). Zwraca id wybranej tury. */
export function selectSibling(tree: TurnTree, turn: Turn, delta: number): string {
  const key = parentKey(turn);
  const list = tree.children[key] ?? [turn.id];
  const current = Math.max(0, list.indexOf(turn.id));
  const next = (((current + delta) % list.length) + list.length) % list.length;
  const id = list[next] ?? turn.id;
  tree.selection[key] = id;
  return id;
}

/** Wybiera konkretną turę (np. nowo dopisany wariant) na jej rozwidleniu. */
export function selectTurn(tree: TurnTree, turn: Turn): void {
  tree.selection[parentKey(turn)] = turn.id;
}

/** Ostatnia tura widocznej ścieżki — rodzic kolejnej wiadomości. */
export function leafId(tree: TurnTree): string | null {
  const path = projectPath(tree);
  return path[path.length - 1]?.id ?? null;
}

export function lastUserTurn(path: readonly Turn[]): Turn | undefined {
  for (let i = path.length - 1; i >= 0; i--) {
    const turn = path[i];
    if (turn?.author === 'user') return turn;
  }
  return undefined;
}

/** Czy w drzewie coś się jeszcze strumieniuje. */
export function streamingTurn(path: readonly Turn[]): Turn | undefined {
  return path.find((turn) => turn.status === 'streaming');
}
