// Druga część słownika (`pl-rest`) przychodzi po starcie okna głównego: kod paczki startowej
// (pliki osiągalne z `src/main.ts` importami statycznymi) nie może używać jej kluczy — inaczej
// w pierwszej klatce zamiast tekstu widać by klucz, a komunikat (toast) zostałby nim na stałe.
import { existsSync, readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { plRest } from '../pl-rest';

const src = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const IMPORT = /^\s*(?:import|export)\s+(?!type\b)(?:[^'"]*?\sfrom\s+)?['"](\.[^'"]+)['"]/gm;

function resolveSpec(from: string, spec: string): string | null {
  const base = resolve(dirname(from), spec);
  for (const p of [base, `${base}.ts`, `${base}.svelte.ts`, `${base}/index.ts`]) {
    if (existsSync(p) && /\.(ts|svelte)$/.test(p)) return p;
  }
  return null;
}

function startupFiles(): string[] {
  const seen = new Set<string>();
  const queue = [resolve(src, 'main.ts')];
  while (queue.length) {
    const file = queue.pop()!;
    if (seen.has(file)) continue;
    seen.add(file);
    for (const m of readFileSync(file, 'utf8').matchAll(IMPORT)) {
      const next = resolveSpec(file, m[1]!);
      if (next) queue.push(next);
    }
  }
  return [...seen].filter((f) => !f.replaceAll('\\', '/').includes('/lib/i18n/'));
}

describe('podział słownika PL', () => {
  const files = startupFiles();
  const restKeys = Object.keys(plRest);

  it('paczka startowa jest rozpoznana (rozmowa, belka, powłoka)', () => {
    const names = files.map((f) => f.replaceAll('\\', '/').split('/').pop());
    for (const name of [
      'AppShell.svelte',
      'TitleBar.svelte',
      'ChatComposer.svelte',
      'Lazy.svelte',
    ]) {
      expect(names).toContain(name);
    }
  });

  it('kod startowy nie używa kluczy z `pl-rest`', () => {
    const problems: string[] = [];
    for (const file of files) {
      const text = readFileSync(file, 'utf8');
      for (const m of text.matchAll(/['"`]([a-zA-Z][\w-]*(?:\.[\w-]+)+)['"`]/g)) {
        if (m[1]! in plRest) problems.push(`${file}: ${m[1]}`);
      }
      // Przedrostki z kropką, także bez kropki na końcu (np. `conv.start.s${n}`).
      for (const m of text.matchAll(/`([a-zA-Z][\w-]*\.[\w.-]*)\$\{/g)) {
        const hit = restKeys.find((k) => k.startsWith(m[1]!));
        if (hit) problems.push(`${file}: ${m[1]}… (np. ${hit})`);
      }
    }
    expect(problems).toEqual([]);
  });
});
