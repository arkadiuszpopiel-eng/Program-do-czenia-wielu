#!/usr/bin/env node
// Budżety paczek (PLAN §14.7, SPEC ui-shell / ui-quick). Uruchom po `pnpm build`; exit 1 przy
// przekroczeniu. Liczy STATYCZNY graf importów każdego punktu wejścia z manifestu Vite (to, co
// przeglądarka musi pobrać przy starcie okna); moduły `import()` są leniwe i raportowane osobno.
//   okno główne (index.html): JS ≤ 150 KB gzip, CSS ≤ 30 KB gzip
//   Szybkie pytanie (quick.html): JS + CSS ≤ 40 KB gzip
//   pigułka głosowa (pill.html): JS + CSS ≤ 8 KB gzip
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';

const dist = join(fileURLToPath(new URL('..', import.meta.url)), 'dist');
const KB = 1024;
const BUDGETS = [
  { entry: 'index.html', name: 'okno główne', js: 150 * KB, css: 30 * KB },
  { entry: 'quick.html', name: 'Szybkie pytanie', total: 40 * KB },
  { entry: 'pill.html', name: 'pigułka głosowa', total: 8 * KB },
];

const manifestPath = join(dist, '.vite', 'manifest.json');
if (!existsSync(manifestPath)) {
  console.error('Brak dist/.vite/manifest.json — uruchom najpierw `pnpm build`.');
  process.exit(1);
}
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
const gzCache = new Map();
const gz = (file) => {
  if (!gzCache.has(file))
    gzCache.set(file, gzipSync(readFileSync(join(dist, file)), { level: 9 }).length);
  return gzCache.get(file);
};
const kb = (n) => `${(n / KB).toFixed(1)} KB`;

/** Statyczne domknięcie importów wpisu: pliki JS i CSS. */
function closure(key) {
  const js = new Set();
  const css = new Set();
  const seen = new Set();
  const visit = (k) => {
    if (seen.has(k)) return;
    seen.add(k);
    const chunk = manifest[k];
    if (!chunk) return;
    if (chunk.file.endsWith('.js')) js.add(chunk.file);
    for (const c of chunk.css ?? []) css.add(c);
    for (const i of chunk.imports ?? []) visit(i);
  };
  visit(key);
  return { js: [...js], css: [...css] };
}

const sum = (files) => files.reduce((total, f) => total + gz(f), 0);
let fail = false;
const startup = new Set();

for (const budget of BUDGETS) {
  const { js, css } = closure(budget.entry);
  if (js.length === 0) {
    console.error(`${budget.entry}: brak wpisu w manifeście`);
    fail = true;
    continue;
  }
  for (const f of [...js, ...css]) startup.add(f);
  const jsSize = sum(js);
  const cssSize = sum(css);
  console.log(`\n▸ ${budget.name} (${budget.entry})`);
  for (const f of [...js, ...css]) console.log(`  ${f.padEnd(44)} ${kb(gz(f)).padStart(9)} gzip`);
  const checks = budget.total
    ? [['JS+CSS', jsSize + cssSize, budget.total]]
    : [
        ['JS', jsSize, budget.js],
        ['CSS', cssSize, budget.css],
      ];
  for (const [label, size, limit] of checks) {
    const ok = size <= limit;
    if (!ok) fail = true;
    console.log(
      `  ${label.padEnd(7)} ${kb(size)} gzip / limit ${kb(limit)} → ${ok ? 'OK' : 'PRZEKROCZONO'}`,
    );
  }
}

// Moduły leniwe (panele, ustawienia, paleta, onboarding, atrapa, słownik EN, worker podświetlania).
const lazy = readdirSync(join(dist, 'assets'))
  .map((name) => `assets/${name}`)
  .filter((f) => !startup.has(f) && statSync(join(dist, f)).isFile() && /\.(js|css)$/.test(f));
console.log('\n▸ moduły leniwe (poza budżetem startowym)');
for (const f of lazy.sort()) console.log(`  ${f.padEnd(44)} ${kb(gz(f)).padStart(9)} gzip`);
console.log(`  razem: ${kb(sum(lazy))} gzip`);

process.exit(fail ? 1 : 0);
