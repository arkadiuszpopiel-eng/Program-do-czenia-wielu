import { describe, expect, it } from 'vitest';
import { tokenize } from '../tokenize';

const kinds = (code: string, lang: string | null) =>
  tokenize(code, lang).map(([s, e, k]) => `${k}:${code.slice(s, e)}`);

describe('tokenizer podświetlania (Web Worker)', () => {
  it('słowa kluczowe, napisy, liczby, komentarze', () => {
    expect(kinds('const x = "a"; // opis', 'ts')).toEqual(['kw:const', 'str:"a"', 'com:// opis']);
    expect(kinds('SELECT 42 -- sql', 'sql')).toEqual(['kw:SELECT', 'num:42', 'com:-- sql']);
    expect(kinds('# py\nreturn 1', 'python')).toEqual(['com:# py', 'kw:return', 'num:1']);
    expect(kinds('/* a */ fn', 'rust')).toEqual(['com:/* a */', 'kw:fn']);
  });

  it('nie koloruje cyfr w identyfikatorach; zakresy nie nachodzą na siebie', () => {
    expect(kinds('abc123 = 5', 'js')).toEqual(['num:5']);
    const ranges = tokenize('let s = "x\\"y"; let n = 0x1F;', 'js');
    for (let i = 1; i < ranges.length; i++)
      expect(ranges[i]?.[0]).toBeGreaterThanOrEqual(ranges[i - 1]?.[1] ?? 0);
  });
});
