import { describe, expect, it } from 'vitest';
import { baseFor, luminance, over, toHex } from '../terminal-colors';

describe('kolory terminala z tokenów motywu', () => {
  it('zapisuje kolor jako #rrggbb', () => {
    expect(toHex({ r: 19, g: 20, b: 23 })).toBe('#131417');
    expect(toHex({ r: 255.4, g: -3, b: 300 })).toBe('#ff00ff');
  });

  it('kładzie kolor półprzezroczysty na podkładzie', () => {
    const half = over({ r: 255, g: 255, b: 255, a: 0.5 }, { r: 0, g: 0, b: 0 });
    expect(toHex(half)).toBe('#808080');
    expect(over({ r: 10, g: 20, b: 30, a: 1 }, { r: 200, g: 200, b: 200 })).toEqual({
      r: 10,
      g: 20,
      b: 30,
      a: 1,
    });
  });

  it('jasny tekst daje ciemny podkład, ciemny tekst — jasny', () => {
    expect(luminance({ r: 237, g: 238, b: 242 })).toBeGreaterThan(0.5);
    expect(toHex(baseFor({ r: 237, g: 238, b: 242 }))).toBe('#131417');
    expect(toHex(baseFor({ r: 26, g: 28, b: 34 }))).toBe('#fafafb');
  });
});
