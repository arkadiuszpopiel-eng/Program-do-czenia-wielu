#!/usr/bin/env node
// Budżet paczki startowej (PLAN.md §14.7): JS ≤ 150 KB gzip, CSS ≤ 30 KB gzip.
// Uruchom po `pnpm build`. Exit 1 przy przekroczeniu.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { gzipSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';

const dist = join(fileURLToPath(new URL('..', import.meta.url)), 'dist');
const LIMITS = { js: 150 * 1024, css: 30 * 1024 };

function* walk(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) yield* walk(p);
    else yield p;
  }
}

const totals = { js: 0, css: 0 };
const rows = [];
for (const file of walk(dist)) {
  const ext = file.endsWith('.js') ? 'js' : file.endsWith('.css') ? 'css' : null;
  if (!ext) continue;
  const raw = readFileSync(file);
  const gz = gzipSync(raw, { level: 9 }).length;
  totals[ext] += gz;
  rows.push({ file: file.slice(dist.length + 1), raw: raw.length, gz });
}

const kb = (n) => `${(n / 1024).toFixed(1)} KB`;
for (const r of rows)
  console.log(`${r.file.padEnd(40)} ${kb(r.raw).padStart(10)} → ${kb(r.gz).padStart(9)} gzip`);
let fail = false;
for (const ext of ['js', 'css']) {
  const ok = totals[ext] <= LIMITS[ext];
  if (!ok) fail = true;
  console.log(
    `${ext.toUpperCase().padEnd(4)} razem: ${kb(totals[ext])} gzip / limit ${kb(LIMITS[ext])} → ${ok ? 'OK' : 'PRZEKROCZONO'}`,
  );
}
process.exit(fail ? 1 : 0);
