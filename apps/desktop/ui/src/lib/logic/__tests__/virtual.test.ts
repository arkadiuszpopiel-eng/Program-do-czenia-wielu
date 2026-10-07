import { describe, expect, it } from 'vitest';
import { VirtualModel, isAtBottom } from '../virtual';

const keys = (n: number) => Array.from({ length: n }, (_, i) => `k${i}`);

describe('VirtualModel — lista wirtualizowana', () => {
  it('renderuje tylko widoczne ± 1 ekran przy 1000 elementach', () => {
    const m = new VirtualModel(100);
    m.setKeys(keys(1000));
    expect(m.totalHeight).toBe(100_000);
    const r = m.range(50_000, 800);
    expect(r.start).toBe(492);
    expect(r.end).toBe(517);
    expect(r.end - r.start).toBeLessThan(30);
  });

  it('pomiary per klucz przeżywają dopisanie elementu na początku', () => {
    const m = new VirtualModel(100);
    m.setKeys(['a', 'b', 'c']);
    expect(m.setHeight('b', 300)).toBe(true);
    expect(m.setHeight('b', 300)).toBe(false);
    m.setKeys(['z', 'a', 'b', 'c']);
    expect(m.offsetOf(3)).toBe(500);
    expect(m.totalHeight).toBe(600);
  });

  it('indexAt i zakres na krańcach', () => {
    const m = new VirtualModel(50);
    expect(m.range(0, 500)).toEqual({ start: 0, end: 0 });
    m.setKeys(keys(3));
    expect(m.indexAt(0)).toBe(0);
    expect(m.indexAt(149)).toBe(2);
    expect(m.indexAt(10_000)).toBe(2);
    expect(m.range(0, 500)).toEqual({ start: 0, end: 3 });
  });

  it('prune usuwa pomiary nieistniejących kluczy', () => {
    const m = new VirtualModel(10);
    m.setKeys(['a', 'b']);
    m.setHeight('a', 40);
    m.setKeys(['b']);
    m.prune();
    m.setKeys(['a', 'b']);
    expect(m.heightOf(0)).toBe(10);
  });

  it('isAtBottom z tolerancją', () => {
    expect(isAtBottom(900, 100, 1000)).toBe(true);
    expect(isAtBottom(870, 100, 1000)).toBe(false);
  });
});
