#!/usr/bin/env node
// Wykrywa składnię Svelte 4 w plikach .svelte (dodatkowa siatka poza ESLint; działa bez
// parsera, więc łapie także pliki, których ESLint nie sparsował).
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const SKIP = new Set(['node_modules', 'dist', 'storybook-static', 'target', '.git']);

const RULES = [
  { re: /^\s*export\s+let\s+/m, msg: '`export let` (użyj $props())' },
  { re: /^\s*\$:\s/m, msg: '`$:` (użyj $derived/$effect)' },
  { re: /\son:[a-zA-Z]+(=|\s|>|\|)/, msg: 'dyrektywa `on:event` (użyj `onevent`)' },
  { re: /createEventDispatcher/, msg: '`createEventDispatcher` (callback przez $props())' },
  { re: /<slot\b/, msg: '`<slot>` (użyj {@render children()})' },
  { re: /\$\$props|\$\$restProps|\$\$slots/, msg: '`$$props/$$restProps/$$slots`' },
  { re: /^\s*import\s+.*\bfrom\s+['"]svelte\/internal/m, msg: 'import z svelte/internal' },
];

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    if (SKIP.has(name)) continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walk(p);
    else if (name.endsWith('.svelte')) yield p;
  }
}

let errors = 0;
let files = 0;
for (const file of walk(ROOT)) {
  files++;
  const src = readFileSync(file, 'utf8');
  for (const { re, msg } of RULES) {
    const m = re.exec(src);
    if (m) {
      const line = src.slice(0, m.index).split('\n').length;
      console.error(`${relative(ROOT, file)}:${line}: Svelte 4: ${msg}`);
      errors++;
    }
  }
}
console.log(`check-svelte5: ${files} plików .svelte, ${errors} naruszeń`);
process.exit(errors ? 1 : 0);
