import { describe, expect, it } from 'vitest';
import { hitsByPage, nextPage, readCollapsed, toggled, writeCollapsed } from '../settings-nav';

describe('nawigacja ustawień', () => {
  it('liczy trafienia per strona; strona trafiona samą nazwą ma zero', () => {
    const hits = hitsByPage(
      [{ page: { id: 'voice' } }, { page: { id: 'voice' } }, { page: { id: 'memory' } }],
      [{ id: 'devices' }, { id: 'voice' }],
    );
    expect([...hits]).toEqual([
      ['devices', 0],
      ['voice', 2],
      ['memory', 1],
    ]);
    expect(hitsByPage([], []).size).toBe(0);
  });

  it('strzałki, Home i End chodzą po liście i zatrzymują się na końcach', () => {
    const ids = ['a', 'b', 'c'];
    expect(nextPage(ids, 'a', 'ArrowDown')).toBe('b');
    expect(nextPage(ids, 'c', 'ArrowDown')).toBe('c');
    expect(nextPage(ids, 'b', 'ArrowUp')).toBe('a');
    expect(nextPage(ids, 'a', 'ArrowUp')).toBe('a');
    expect(nextPage(ids, 'b', 'Home')).toBe('a');
    expect(nextPage(ids, 'b', 'End')).toBe('c');
    // Fokus poza listą: w dół = pierwsza pozycja.
    expect(nextPage(ids, 'x', 'ArrowDown')).toBe('a');
    expect(nextPage(ids, 'a', 'Enter')).toBeNull();
    expect(nextPage([], 'a', 'ArrowDown')).toBeNull();
  });

  it('zwinięte grupy przeżywają zamknięcie okna, a zepsuta pamięć niczego nie psuje', () => {
    const data = new Map<string, string>();
    const storage = {
      getItem: (key: string) => data.get(key) ?? null,
      setItem: (key: string, value: string) => void data.set(key, value),
    };
    expect(readCollapsed(storage)).toEqual([]);
    writeCollapsed(storage, toggled(toggled([], 'voice'), 'data'));
    expect(readCollapsed(storage)).toEqual(['data', 'voice']);
    expect(toggled(['data', 'voice'], 'voice')).toEqual(['data']);
    for (const broken of ['{', '"voice"', '[1, "safety", null]']) {
      data.set('alfa.settings.nav.collapsed', broken);
      expect(readCollapsed(storage)).toEqual(broken.startsWith('[') ? ['safety'] : []);
    }
    expect(readCollapsed(null)).toEqual([]);
    const failing = {
      setItem: () => {
        throw new Error('pełna');
      },
    };
    expect(() => writeCollapsed(failing, ['voice'])).not.toThrow();
  });
});
