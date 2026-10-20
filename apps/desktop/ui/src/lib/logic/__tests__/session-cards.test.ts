import { describe, expect, it } from 'vitest';
import {
  SNIPPET_CHARS,
  columnsOf,
  knownAgents,
  moveInGrid,
  plainSnippet,
  readSessionsView,
  writeSessionsView,
} from '../session-cards';

function memory(): Storage {
  const data = new Map<string, string>();
  return {
    get length() {
      return data.size;
    },
    clear: () => data.clear(),
    getItem: (key) => data.get(key) ?? null,
    key: (i) => [...data.keys()][i] ?? null,
    removeItem: (key) => void data.delete(key),
    setItem: (key, value) => void data.set(key, value),
  };
}

describe('widok panelu Sesje', () => {
  it('domyślnie lista; zapamiętane karty wracają', () => {
    const storage = memory();
    expect(readSessionsView(storage)).toBe('list');
    writeSessionsView(storage, 'cards');
    expect(readSessionsView(storage)).toBe('cards');
    writeSessionsView(storage, 'list');
    expect(readSessionsView(storage)).toBe('list');
  });

  it('brak pamięci albo błąd — lista, bez wyjątku', () => {
    expect(readSessionsView(null)).toBe('list');
    const broken = {
      getItem: () => {
        throw new Error('zablokowane');
      },
      setItem: () => {
        throw new Error('zablokowane');
      },
    };
    expect(readSessionsView(broken)).toBe('list');
    expect(() => writeSessionsView(broken, 'cards')).not.toThrow();
  });

  it('nieznana wartość w pamięci — lista', () => {
    const storage = memory();
    storage.setItem('alfa.sessions.view', 'kafelki');
    expect(readSessionsView(storage)).toBe('list');
  });
});

describe('ruch po siatce kart', () => {
  it('strzałki w dwóch kolumnach', () => {
    expect(moveInGrid(0, 'ArrowRight', 2, 5)).toBe(1);
    expect(moveInGrid(1, 'ArrowLeft', 2, 5)).toBe(0);
    expect(moveInGrid(1, 'ArrowDown', 2, 5)).toBe(3);
    expect(moveInGrid(3, 'ArrowUp', 2, 5)).toBe(1);
    expect(moveInGrid(3, 'End', 2, 5)).toBe(4);
    expect(moveInGrid(3, 'Home', 2, 5)).toBe(0);
  });

  it('jedna kolumna (wąskie okno): w dół i w górę po kolei', () => {
    expect(moveInGrid(0, 'ArrowDown', 1, 3)).toBe(1);
    expect(moveInGrid(2, 'ArrowUp', 1, 3)).toBe(1);
  });

  it('na krawędzi zostaje w miejscu; inne klawisze nie przesuwają', () => {
    expect(moveInGrid(0, 'ArrowLeft', 2, 4)).toBe(0);
    expect(moveInGrid(3, 'ArrowDown', 2, 4)).toBe(3);
    expect(moveInGrid(2, 'ArrowDown', 2, 3)).toBe(2);
    expect(moveInGrid(1, 'Enter', 2, 4)).toBeNull();
    expect(moveInGrid(0, 'ArrowDown', 2, 0)).toBeNull();
    expect(moveInGrid(0, 'ArrowDown', 0, 3)).toBe(1);
  });

  it('kolumny z górnych krawędzi kart', () => {
    expect(columnsOf([])).toBe(1);
    expect(columnsOf([10])).toBe(1);
    expect(columnsOf([10, 10.5, 120, 120])).toBe(2);
    expect(columnsOf([10, 120, 230])).toBe(1);
  });
});

describe('agentki na karcie', () => {
  it('tylko znane, bez powtórzeń, do czterech', () => {
    expect(knownAgents(['beta', 'nieznana', 'beta', 'alfa'])).toEqual(['beta', 'alfa']);
    expect(knownAgents(['alfa', 'beta', 'gama', 'delta', 'alfa'])).toHaveLength(4);
    expect(knownAgents([])).toEqual([]);
  });
});

describe('czysty tekst ostatniej wiadomości', () => {
  it('bez Markdownu i HTML, w jednej linii', () => {
    expect(plainSnippet('# Tytuł\n\n- **punkt** z [linkiem](https://x.pl)\n- `kod`')).toBe(
      'Tytuł punkt z linkiem kod',
    );
    expect(plainSnippet('<img src=x onerror=alert(1)>Tekst <b>gruby</b>')).toBe('Tekst gruby');
    expect(plainSnippet('Przed\n```js\nalert(1)\n```\npo')).toBe('Przed (kod) po');
  });

  it('bez znaków sterujących i niewidocznych', () => {
    const rlo = String.fromCodePoint(0x202e);
    const zwsp = String.fromCodePoint(0x200b);
    expect(plainSnippet(`a\tb${String.fromCodePoint(7)}c ${rlo}d${zwsp}e`)).toBe('a b c de');
  });

  it('długi tekst ucięty z wielokropkiem', () => {
    const out = plainSnippet('słowo '.repeat(100));
    expect(Array.from(out)).toHaveLength(SNIPPET_CHARS);
    expect(out.endsWith('…')).toBe(true);
    expect(plainSnippet('krótki', 3)).toBe('kr…');
  });
});
