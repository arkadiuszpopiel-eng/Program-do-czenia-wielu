#!/usr/bin/env node
// Zakaz `@import url(` (fonty webowe / zdalne CSS) i `backdrop-filter` (poza paletą poleceń).
// Reguła twarda z AGENTS.md; PLAN.md §14.3 / §14.7.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { basename, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

// fileURLToPath: `.pathname` psuje ścieżki Windows (`/E:/…`) i ze spacjami (`%20`).
const ROOT = fileURLToPath(new URL('..', import.meta.url));
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

// Zmienne `--alfa-*` użyte przez `var(...)` bez wartości zapasowej muszą być gdzieś zdefiniowane:
// literówka w nazwie nie daje błędu, tylko po cichu wyłącza regułę (kanciasty przycisk, zerowy odstęp).
// Definicją jest deklaracja w CSS, `style:--alfa-x` w Svelte albo `setProperty('--alfa-x', …)` w skrypcie.
const DEFINED =
  /(?:^|[\s;{"'(:])(--alfa-[a-z0-9-]+)\s*[:=,]|setProperty\(\s*['"](--alfa-[a-z0-9-]+)['"]/gm;
const USED = /var\(\s*(--alfa-[a-z0-9-]+)\s*\)/g;
const defined = new Set();
const sources = [];
function* walkAll(dir) {
  for (const name of readdirSync(dir)) {
    if (SKIP.has(name) || name === 'html') continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walkAll(p);
    else if (/\.(css|svelte|html|ts)$/.test(name)) yield p;
  }
}
for (const dir of ['apps/desktop/ui/src', 'apps/desktop/ui/public', 'packages/ui-kit/src']) {
  for (const file of walkAll(join(ROOT, dir))) {
    const src = readFileSync(file, 'utf8');
    sources.push([file, src]);
    for (const m of src.matchAll(DEFINED)) defined.add(m[1] ?? m[2]);
  }
}

let errors = 0;
for (const [file, src] of sources) {
  for (const m of src.matchAll(USED)) {
    if (defined.has(m[1])) continue;
    const line = src.slice(0, m.index).split('\n').length;
    console.error(
      `${relative(ROOT, file)}:${line}: zmienna \`${m[1]}\` nie jest nigdzie zdefiniowana`,
    );
    errors++;
  }
}

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
