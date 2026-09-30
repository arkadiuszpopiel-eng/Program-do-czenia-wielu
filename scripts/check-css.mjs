#!/usr/bin/env node
// Zakaz `@import url(` (fonty webowe / zdalne CSS) i `backdrop-filter` (poza paletą poleceń).
// Reguła twarda z AGENTS.md; PLAN.md §14.3 / §14.7.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { basename, join, relative } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const SKIP = new Set(['node_modules', 'dist', 'storybook-static', 'target', '.git']);
const EXT = /\.(css|svelte|html)$/;
const BACKDROP_ALLOWED = new Set(['CommandPalette.svelte']);

const RULES = [
  { re: /@import\s+url\(/, msg: '`@import url(` — zero fontów webowych i zdalnego CSS' },
  { re: /@import\s+["']https?:/, msg: '`@import "http…"` — zero zdalnego CSS' },
  { re: /fonts\.googleapis\.com|fonts\.gstatic\.com|use\.typekit\.net/, msg: 'web font' },
  { re: /@font-face/, msg: '`@font-face` — tylko fonty systemowe' },
  {
    re: /backdrop-filter\s*:/,
    msg: '`backdrop-filter` (dozwolone tylko w CommandPalette.svelte)',
    allow: BACKDROP_ALLOWED,
  },
];

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    if (SKIP.has(name)) continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walk(p);
    else if (EXT.test(name)) yield p;
  }
}

let errors = 0;
let files = 0;
for (const file of walk(ROOT)) {
  files++;
  const src = readFileSync(file, 'utf8');
  for (const { re, msg, allow } of RULES) {
    if (allow?.has(basename(file))) continue;
    const m = re.exec(src);
    if (m) {
      const line = src.slice(0, m.index).split('\n').length;
      console.error(`${relative(ROOT, file)}:${line}: ${msg}`);
      errors++;
    }
  }
}
console.log(`check-css: ${files} plików, ${errors} naruszeń`);
process.exit(errors ? 1 : 0);
