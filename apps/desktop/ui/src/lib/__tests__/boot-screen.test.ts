import { describe, expect, it } from 'vitest';
import { mouthOpen } from '@alfa/ui-kit';
import { applyRememberedTheme, rememberTheme, remainingBootTime } from '../boot-screen';

describe('ekran ładowania', () => {
  it('trzyma się na ekranie co najmniej zadany czas, ale nie dłużej niż trzeba', () => {
    expect(remainingBootTime(100, 0, 700)).toBe(600);
    expect(remainingBootTime(700, 0, 700)).toBe(0);
    expect(remainingBootTime(5_000, 0, 700)).toBe(0);
    expect(remainingBootTime(50, 0, 0)).toBe(0);
  });

  it('pamięta motyw jasny albo ciemny; „auto” zostawia motyw systemu', () => {
    const data = new Map<string, string>();
    const storage = {
      getItem: (key: string) => data.get(key) ?? null,
      setItem: (key: string, value: string) => void data.set(key, value),
    };
    const attrs = new Map<string, string>();
    const root = {
      setAttribute: (name: string, value: string) => void attrs.set(name, value),
    } as unknown as HTMLElement;
    applyRememberedTheme(root, storage);
    expect(attrs.size).toBe(0);
    rememberTheme(storage, 'dark');
    applyRememberedTheme(root, storage);
    expect(attrs.get('data-theme')).toBe('dark');
    attrs.clear();
    rememberTheme(storage, 'auto');
    applyRememberedTheme(root, storage);
    expect(attrs.size).toBe(0);
    // Niedostępna pamięć przeglądarki niczego nie psuje.
    const broken = {
      getItem: () => {
        throw new Error('zablokowana');
      },
      setItem: () => {
        throw new Error('zablokowana');
      },
    };
    expect(() => applyRememberedTheme(root, broken)).not.toThrow();
    expect(() => rememberTheme(broken, 'dark')).not.toThrow();
  });
});

describe('usta awatara', () => {
  it('cisza zamyka usta, cicha mowa jest widoczna, głośna nie wychodzi poza zakres', () => {
    expect(mouthOpen(0)).toBe(0);
    expect(mouthOpen(0.01)).toBe(0);
    expect(mouthOpen(0.1)).toBeGreaterThan(0.35);
    expect(mouthOpen(0.5)).toBeGreaterThan(mouthOpen(0.1));
    expect(mouthOpen(1)).toBe(1);
    expect(mouthOpen(7)).toBe(1);
    expect(mouthOpen(-3)).toBe(0);
  });
});
