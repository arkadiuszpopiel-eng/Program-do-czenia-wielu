import { describe, expect, it } from 'vitest';
import type { Turn } from '../../api/types';
import { turn } from '../../api/fake/fixtures';
import {
  addTurn,
  buildTree,
  leafId,
  lastUserTurn,
  projectPath,
  selectSibling,
  selectTurn,
  siblingInfo,
} from '../turn-tree';

const T0 = Date.UTC(2026, 8, 30, 10);
const t = (id: string, parent: string | null, author: Turn['author'], i: number) =>
  turn(id, 's', parent, author, id, T0 + i * 1000);

describe('drzewo gałęzi (append-only)', () => {
  const base = () =>
    buildTree([
      t('u1', null, 'user', 0),
      t('a1', 'u1', 'alfa', 1),
      t('a1b', 'u1', 'alfa', 2),
      t('u2', 'a1b', 'user', 3),
    ]);

  it('projekcja: najnowszy wariant domyślnie', () => {
    expect(projectPath(base()).map((x) => x.id)).toEqual(['u1', 'a1b', 'u2']);
  });

  it('‹ 1/2 › — przełączenie wariantu zmienia dalszą ścieżkę', () => {
    const tree = base();
    const a1b = tree.turns['a1b'];
    expect(a1b && siblingInfo(tree, a1b)).toEqual({ index: 2, total: 2 });
    if (!a1b) throw new Error('brak');
    selectSibling(tree, a1b, -1);
    expect(projectPath(tree).map((x) => x.id)).toEqual(['u1', 'a1']);
    const a1 = tree.turns['a1'];
    if (!a1) throw new Error('brak');
    selectSibling(tree, a1, 1);
    expect(projectPath(tree).map((x) => x.id)).toEqual(['u1', 'a1b', 'u2']);
  });

  it('edytuj → nowa gałąź obok, bez modyfikacji istniejących tur', () => {
    const tree = base();
    const before = JSON.stringify(tree.turns);
    const edited = t('u2b', 'a1b', 'user', 4);
    expect(addTurn(tree, edited)).toBe(true);
    selectTurn(tree, edited);
    expect(projectPath(tree).map((x) => x.id)).toEqual(['u1', 'a1b', 'u2b']);
    const parsed = JSON.parse(before) as Record<string, Turn>;
    for (const id of Object.keys(parsed)) expect(tree.turns[id]).toEqual(parsed[id]);
    expect(leafId(tree)).toBe('u2b');
  });

  it('addTurn jest idempotentne', () => {
    const tree = base();
    const again = tree.turns['a1'];
    expect(again && addTurn(tree, again)).toBe(false);
    expect(tree.children['u1']).toEqual(['a1', 'a1b']);
  });

  it('lastUserTurn i pusta rozmowa', () => {
    const tree = base();
    expect(lastUserTurn(projectPath(tree))?.id).toBe('u2');
    expect(leafId(buildTree([]))).toBeNull();
  });

  it('property: dowolna sekwencja dopisań daje spójną ścieżkę', () => {
    let seed = 7;
    const rnd = () => (seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31;
    for (let round = 0; round < 50; round++) {
      const turns: Turn[] = [];
      for (let i = 0; i < 30; i++) {
        const parent = turns.length && rnd() > 0.1 ? turns[Math.floor(rnd() * turns.length)] : null;
        turns.push(t(`n${i}`, parent?.id ?? null, i % 2 ? 'gama' : 'user', i));
      }
      const tree = buildTree(turns);
      const path = projectPath(tree);
      path.forEach((node, i) => {
        expect(node.parent_id).toBe(i === 0 ? null : path[i - 1]?.id);
      });
      const leaf = path[path.length - 1];
      expect(leaf && (tree.children[leaf.id] ?? []).length).toBe(0);
    }
  });
});
