// Trusted Types z ui-kit (PT-33): bez Trusted Types w środowisku — zwykłe łańcuchy; polityka
// `default` przepuszcza tylko adresy skryptów z tego samego pochodzenia (worker podświetlania),
// a HTML z backendu idzie wyłącznie przez `alfa-sanitized-html`. Pakiet ui-kit nie ma własnego
// runnera testów — logika bez DOM jest sprawdzana tutaj; zachowanie pod CSP sprawdza E2E
// (`vite preview` z CSP wydania).
import { afterEach, describe, expect, it, vi } from 'vitest';
import { sameOriginScriptUrl, sanitizedHtml } from '@alfa/ui-kit';

type Rules = { createHTML?: (s: string) => string; createScriptURL?: (s: string) => string };

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
});

describe('Trusted Types', () => {
  it('bez Trusted Types HTML przechodzi bez zmian', () => {
    expect(sanitizedHtml('<p>a</p>')).toBe('<p>a</p>');
  });

  it('adres skryptu tylko z tego samego pochodzenia', () => {
    const page = 'http://tauri.localhost/index.html';
    expect(sameOriginScriptUrl('/assets/highlight.worker-x.js', page)).toBe(
      '/assets/highlight.worker-x.js',
    );
    expect(sameOriginScriptUrl('http://tauri.localhost/assets/w.js', page)).toBe(
      'http://tauri.localhost/assets/w.js',
    );
    for (const bad of [
      'https://evil.example/w.js',
      '//evil.example/w.js',
      'data:text/javascript,alert(1)',
      'http://tauri.localhost.evil.example/w.js',
    ]) {
      expect(() => sameOriginScriptUrl(bad, page), bad).toThrow(TypeError);
    }
  });

  it('polityki: HTML przez alfa-sanitized-html, default bez HTML i skryptów', async () => {
    const created = new Map<string, Rules>();
    vi.stubGlobal('location', { href: 'http://tauri.localhost/index.html' });
    vi.stubGlobal('trustedTypes', {
      createPolicy(name: string, rules: Rules) {
        if (created.has(name)) throw new TypeError(`powtórzona polityka ${name}`);
        created.set(name, rules);
        return {
          createHTML: rules.createHTML && ((s: string) => ({ trusted: rules.createHTML?.(s) })),
          createScriptURL: rules.createScriptURL,
        };
      },
    });
    vi.resetModules();
    const kit = await import('@alfa/ui-kit');
    expect(kit.sanitizedHtml('<b>x</b>')).toEqual({ trusted: '<b>x</b>' });
    kit.installDefaultPolicy();
    kit.installDefaultPolicy();
    const fallback = created.get('default');
    expect(fallback?.createHTML).toBeUndefined();
    expect(fallback?.createScriptURL?.('/assets/w.js')).toBe('/assets/w.js');
    expect(() => fallback?.createScriptURL?.('https://evil.example/w.js')).toThrow(TypeError);
    expect([...created.keys()].sort()).toEqual(['alfa-sanitized-html', 'default']);
  });
});
